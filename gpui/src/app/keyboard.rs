//! 键盘页面：总览里的键盘卡片 + 键盘详情页。
//!
//! 这是 `app` 的子模块（不是独立 crate），所以能直接读写 `AppView` 的私有
//! 字段；键盘本身的协议在 `openmouse_keyboard` 里，这里只做状态与界面。
//!
//! 一条设计约束：**不长期持有设备句柄**。`hidapi::HidDevice` 不适合跨线程
//! 存活，而 `AppView` 会被 GPUI 的实体要求可发送。所以每个操作都是「打开 →
//! 做事 → 关闭」，在后台执行器里跑完再把结果送回界面。打开设备的开销是毫秒
//! 级，用户感知不到，换来的是不用处理句柄生命周期和重连。

use super::AppView;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{h_flex, ActiveTheme as _, Disableable as _};
use gpui_kit::*;
use openmouse_keyboard::{
    effect_mode, effect_mode_name, keymap, Action, GameMode, KeyBinding, Keyboard, LedEffect,
    Macro, Page as KeyPage, EFFECT_MODES, NUT75_LAYOUT,
};

/// 键盘详情页的标签。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum KbTab {
    /// 改键。
    #[default]
    Keys,
    /// 灯效。
    Lighting,
    /// 宏。
    Macro,
    /// 高级键（SOCD / MT / TGL / CB）。
    Advanced,
    /// 设置。
    Settings,
}

impl KbTab {
    const ALL: [Self; 5] = [Self::Keys, Self::Lighting, Self::Macro, Self::Advanced, Self::Settings];

    fn label(self) -> &'static str {
        match self {
            Self::Keys => "改键",
            Self::Lighting => "灯效",
            Self::Macro => "宏",
            Self::Advanced => "高级键",
            Self::Settings => "设置",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Keys => "kb-tab-keys",
            Self::Lighting => "kb-tab-lighting",
            Self::Macro => "kb-tab-macro",
            Self::Advanced => "kb-tab-advanced",
            Self::Settings => "kb-tab-settings",
        }
    }
}

/// 一次读到的键盘全量状态。
struct Snapshot {
    connection: &'static str,
    firmware: f32,
    battery: u8,
    keys: Vec<KeyBinding>,
    fn_keys: Vec<KeyBinding>,
    led: Option<LedEffect>,
    settings: Option<GameMode>,
    macros: Vec<Macro>,
    /// 单项读取失败的原因，界面上标出来但不影响其它项。
    partial: Vec<String>,
}

/// 打开键盘、把各模块读一遍、关掉。
///
/// 在后台执行器里跑，所以这里用阻塞调用没关系。任何一项失败都只记录原因，
/// 不中断——比如侧灯没启用时读出来全是 0，这不该让整个页面打不开。
fn read_snapshot() -> Result<Snapshot, String> {
    let kb = Keyboard::open().map_err(friendly_error)?;
    let info = kb.info();
    let mut partial = Vec::new();

    let keys = match kb.keymap() {
        Ok(keys) => keys,
        Err(e) => {
            partial.push(format!("键位表读取失败：{e}"));
            Vec::new()
        }
    };
    let fn_keys = match kb.fn_keys() {
        Ok(keys) => keys,
        Err(e) => {
            partial.push(format!("FN 层读取失败：{e}"));
            Vec::new()
        }
    };
    let led = match kb.led_effect() {
        Ok(value) => Some(value),
        Err(e) => {
            partial.push(format!("灯效读取失败：{e}"));
            None
        }
    };
    let settings = match kb.game_mode() {
        Ok(value) => Some(value),
        Err(e) => {
            partial.push(format!("设置读取失败：{e}"));
            None
        }
    };
    let macros = match kb.macros() {
        Ok(value) => value,
        Err(e) => {
            partial.push(format!("宏读取失败：{e}"));
            Vec::new()
        }
    };

    Ok(Snapshot {
        connection: info.connection(),
        firmware: info.firmware,
        battery: info.battery,
        keys,
        fn_keys,
        led,
        settings,
        macros,
        partial,
    })
}

/// 把底层错误翻译成用户能照做的提示。
///
/// 最常见的一种是键盘休眠：NUT75 闲置一段时间后 2.4G 会断开配置通道，此时
/// `GET_DEVICE_INFO` 收不到应答。报"命令无应答"用户看不懂，说清楚按一下键
/// 就能恢复才有用。
fn friendly_error(error: openmouse_keyboard::Error) -> String {
    match &error {
        openmouse_keyboard::Error::Timeout { cmd } if *cmd == 0x10 => {
            "键盘没有应答。多半是闲置后进入了休眠，按一下键盘上的任意键唤醒，再点「重新读取」。"
                .to_string()
        }
        openmouse_keyboard::Error::NoDevice => {
            "没找到键盘。请确认用 USB 线或 2.4G 接收器连接（蓝牙模式没有配置通道）。".to_string()
        }
        _ => error.to_string(),
    }
}

/// 键盘在界面里的状态。
pub(crate) struct KeyboardState {
    /// 是否正在扫描。
    pub scanning: bool,
    /// 是否找到了键盘。
    pub present: bool,
    /// 详情页是否打开。
    pub open: bool,
    pub tab: KbTab,
    pub error: Option<String>,
    pub connection: String,
    pub firmware: f32,
    pub battery: u8,
    pub keys: Vec<KeyBinding>,
    pub fn_keys: Vec<KeyBinding>,
    pub led: Option<LedEffect>,
    pub settings: Option<GameMode>,
    pub macros: Vec<Macro>,
    pub partial: Vec<String>,
    /// 正在写入的标签，用于禁用重复操作。
    pub writing: Option<String>,
    pub write_error: Option<String>,
    /// 改键页里选中的键位。
    pub selected_key: Option<usize>,
    /// FN 层视图（改键页的切换）。
    pub show_fn_layer: bool,
    /// 宏页里正在编辑的槽位。
    pub macro_slot: Option<u8>,
    /// 编辑中的动作草稿。和键盘里的数据分开，点「保存」才写下去。
    pub macro_draft: Vec<Action>,
    /// 草稿是否改过（决定要不要提示"未保存"）。
    pub macro_dirty: bool,
}

impl Default for KeyboardState {
    fn default() -> Self {
        Self {
            scanning: false,
            present: false,
            open: false,
            tab: KbTab::default(),
            error: None,
            connection: String::new(),
            firmware: 0.0,
            battery: 0,
            keys: Vec::new(),
            fn_keys: Vec::new(),
            led: None,
            settings: None,
            macros: Vec::new(),
            partial: Vec::new(),
            writing: None,
            write_error: None,
            selected_key: None,
            show_fn_layer: false,
            macro_slot: None,
            macro_draft: Vec::new(),
            macro_dirty: false,
        }
    }
}

impl KeyboardState {
    fn apply(&mut self, result: Result<Snapshot, String>) {
        self.scanning = false;
        match result {
            Ok(snapshot) => {
                self.present = true;
                self.error = None;
                self.connection = snapshot.connection.to_string();
                self.firmware = snapshot.firmware;
                self.battery = snapshot.battery;
                self.keys = snapshot.keys;
                self.fn_keys = snapshot.fn_keys;
                self.led = snapshot.led;
                self.settings = snapshot.settings;
                self.macros = snapshot.macros;
                self.partial = snapshot.partial;
            }
            Err(error) => {
                eprintln!("键盘读取失败: {error}");
                self.present = false;
                self.error = Some(error);
            }
        }
    }

    /// 当前视图对应的键位表。
    fn active_keys(&self) -> &[KeyBinding] {
        if self.show_fn_layer {
            &self.fn_keys
        } else {
            &self.keys
        }
    }

    /// 有绑定的键位数量。
    ///
    /// 注意这不是"改过"的数量：NUT75 对每个物理键都显式存了
    /// `[2, 0, usage, 0]`，所以 113 个位置全都有值，看起来都"非默认"。
    fn bound_count(&self) -> usize {
        self.active_keys().iter().filter(|k| !k.is_default()).count()
    }

    /// 真正被改成非键盘功能的键位数量。
    ///
    /// 判据是页面类型：还是键盘页（或空）说明只是原样存着，改成鼠标、多媒体、
    /// 宏、高级键才算动过。
    fn customized_count(&self) -> usize {
        self.active_keys()
            .iter()
            .filter(|k| !matches!(k.page(), KeyPage::Default | KeyPage::Keyboard))
            .count()
    }
}

impl AppView {
    /// 扫一遍键盘。应用启动和点「重新扫描」时各调一次。
    pub(crate) fn scan_keyboard(&mut self, cx: &mut Context<Self>) {
        if self.keyboard.scanning {
            return;
        }
        self.keyboard.scanning = true;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let result = cx.background_executor().spawn(async move { read_snapshot() }).await;
            let _ = view.update(cx, |this: &mut Self, cx| {
                this.keyboard.apply(result);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_keyboard(&mut self, cx: &mut Context<Self>) {
        self.keyboard.open = true;
        self.keyboard.write_error = None;
        // 每次进来重读一次，电量这类会变的值得刷新
        self.scan_keyboard(cx);
        cx.notify();
    }

    /// 开发用：绕过鼠标点击，直接从命令行打开键盘页并选中某个标签。
    ///
    /// 配合 `OPENMOUSE_START_PAGE=keyboard` 与 `OPENMOUSE_KB_TAB=lighting`
    /// 这类参数使用，方便逐页截图或压测，不依赖窗口焦点和 DPI 缩放下的点击
    /// 坐标。
    /// 开发用自检：等页面读完数据后，真的走一次"改设置"的写入链路。
    ///
    /// 由 `OPENMOUSE_KB_SELFTEST=key_delay` 触发。它调用的是和点界面 chip
    /// 完全相同的 `kb_edit_settings`，所以能验证 `kb_write` 的后台任务、设备
    /// 写入、读回刷新这一整条链路，而不依赖鼠标点击（合成点击在 DPI 缩放下
    /// 坐标不可靠）。
    fn kb_run_selftest(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |view, cx| {
            // 给启动时的第一次读取留出时间
            cx.background_executor().timer(std::time::Duration::from_secs(3)).await;
            let _ = view.update(cx, |this: &mut Self, cx| {
                let current = this.keyboard.settings.map(|s| s.key_delay);
                let target = match current {
                    Some(3) => 4,
                    _ => 3,
                };
                eprintln!("自检：按键延时 {current:?} -> {target}");
                this.kb_edit_settings("自检", move |s| s.key_delay = target, cx);
            });
        })
        .detach();
    }

    pub fn show_keyboard_at(&mut self, tab: Option<&str>, cx: &mut Context<Self>) {
        if let Some(tab) = tab {
            self.keyboard.tab = match tab {
                "lighting" => KbTab::Lighting,
                "macro" => KbTab::Macro,
                "advanced" => KbTab::Advanced,
                "settings" => KbTab::Settings,
                _ => KbTab::Keys,
            };
        }
        self.open_keyboard(cx);
        if std::env::var("OPENMOUSE_KB_SELFTEST").is_ok() {
            self.kb_run_selftest(cx);
        }
    }

    pub(crate) fn close_keyboard(&mut self, cx: &mut Context<Self>) {
        self.keyboard.open = false;
        self.keyboard.selected_key = None;
        self.keyboard.write_error = None;
        cx.notify();
    }

    /// 跑一次写入，写完后重读并刷新界面。
    ///
    /// 界面上先乐观更新，写入失败时那次重读会把真实状态拉回来，所以不会出现
    /// 「看着改了其实没改」的错觉。
    fn kb_write(
        &mut self,
        label: impl Into<String>,
        cx: &mut Context<Self>,
        job: impl FnOnce() -> Result<(), String> + Send + 'static,
    ) {
        if self.keyboard.writing.is_some() {
            return;
        }
        let label = label.into();
        eprintln!("键盘写入开始: {label}");
        self.keyboard.writing = Some(label.clone());
        self.keyboard.write_error = None;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let (outcome, snapshot) = cx
                .background_executor()
                .spawn(async move {
                    let outcome = job();
                    let snapshot = read_snapshot();
                    (outcome, snapshot)
                })
                .await;
            match &outcome {
                Ok(()) => eprintln!("键盘写入成功: {label}"),
                Err(error) => eprintln!("键盘写入失败: {label}: {error}"),
            }
            let _ = view.update(cx, |this: &mut Self, cx| {
                this.keyboard.writing = None;
                this.keyboard.write_error = outcome.err();
                this.keyboard.apply(snapshot);
                cx.notify();
            });
        })
        .detach();
    }

    /// 把某个键位改成 `binding`（`None` = 恢复出厂默认）。
    pub(crate) fn kb_assign_key(
        &mut self,
        index: usize,
        binding: Option<KeyBinding>,
        cx: &mut Context<Self>,
    ) {
        let fn_layer = self.keyboard.show_fn_layer;
        let source = self.keyboard.active_keys().to_vec();
        if source.len() <= index {
            return;
        }
        let mut next = source;
        next[index] = binding.unwrap_or_else(keymap::factory_default);
        if fn_layer {
            self.keyboard.fn_keys = next.clone();
        } else {
            self.keyboard.keys = next.clone();
        }

        let label = format!("{}键位 {}", if fn_layer { "FN 层 " } else { "" }, index);
        self.kb_write(label, cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            if fn_layer {
                kb.set_fn_keys(&next).map_err(|e| e.to_string())
            } else {
                kb.set_keymap(&next).map_err(|e| e.to_string())
            }
        });
    }

    /// 改一个设置字段。
    pub(crate) fn kb_edit_settings(
        &mut self,
        label: impl Into<String>,
        change: impl FnOnce(&mut GameMode),
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.keyboard.settings else { return };
        let next = current.with(change);
        self.keyboard.settings = Some(next);
        self.kb_write(label, cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            kb.set_game_mode(&next).map_err(|e| e.to_string())
        });
    }

    /// 改回报率。
    ///
    /// 挑哪个字段由连接方式决定（2.4G 且无线字段已是 Hz → 写无线字段，否则
    /// 写档位码），规则跟官方驱动一样，见 `GameMode::with_report_rate`。写完照
    /// 例重读，读回来的值才是准的——回报率是少数「写了也可能被设备忽略」的项。
    pub(crate) fn kb_set_report_rate(&mut self, hz: u32, cx: &mut Context<Self>) {
        let Some(current) = self.keyboard.settings else { return };
        let is_wireless = self.keyboard.connection.contains("2.4G");
        let next = current.with_report_rate(hz, is_wireless);
        self.keyboard.settings = Some(next);
        self.kb_write(format!("回报率 {hz} Hz"), cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            kb.set_game_mode(&next).map_err(|e| e.to_string())
        });
    }

    /// 改一个灯效字段（模式、亮度、速度、颜色都走这条路）。
    ///
    /// 写回是**无损**的：`LedEffect` 内部留着读回来的 16 字节原始值，`encode()`
    /// 以它为底只覆盖改动的字段，所以 [13] 这种没名字的校验位不会被清掉。
    pub(crate) fn kb_edit_lighting(
        &mut self,
        label: impl Into<String>,
        change: impl FnOnce(&mut LedEffect),
        cx: &mut Context<Self>,
    ) {
        let Some(current) = self.keyboard.led else { return };
        let next = current.with(change);
        self.keyboard.led = Some(next);
        self.kb_write(label, cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            kb.set_led_effect(&next).map_err(|e| e.to_string())
        });
    }

    // ---- 宏编辑 ----
    //
    // 宏是**两段式写入**（400 字节索引表 + 数据区），而界面上一次只编辑一个
    // 槽。所以保存时不是写"这一个宏"，而是把「键盘上现存的宏 + 这个槽的新内容」
    // 整表重写一遍——官方驱动的 setMacroData 也是整表写。这样槽位地址会自动
    // 重排，删掉中间的宏也不会留下空洞。

    /// 选中一个槽开始编辑，草稿从键盘读到的内容拷一份。
    pub(crate) fn kb_pick_macro_slot(&mut self, slot: u8, cx: &mut Context<Self>) {
        self.keyboard.macro_slot = Some(slot);
        self.keyboard.macro_draft = self
            .keyboard
            .macros
            .iter()
            .find(|entry| entry.id == slot)
            .map(|entry| entry.actions.clone())
            .unwrap_or_default();
        self.keyboard.macro_dirty = false;
        self.keyboard.write_error = None;
        cx.notify();
    }

    /// 追加一个按键：按下 + 抬起两条，间隔 50ms。
    pub(crate) fn kb_macro_append_key(&mut self, usage: u8, cx: &mut Context<Self>) {
        self.keyboard.macro_draft.push(Action::keyboard(usage, true, 50));
        self.keyboard.macro_draft.push(Action::keyboard(usage, false, 50));
        self.keyboard.macro_dirty = true;
        cx.notify();
    }

    /// 追加一个鼠标键：按下 + 抬起。
    pub(crate) fn kb_macro_append_mouse(&mut self, button: u8, cx: &mut Context<Self>) {
        self.keyboard.macro_draft.push(Action::mouse(button, true, 50));
        self.keyboard.macro_draft.push(Action::mouse(button, false, 50));
        self.keyboard.macro_dirty = true;
        cx.notify();
    }

    /// 翻转某条动作的按下/抬起。
    pub(crate) fn kb_macro_toggle(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(action) = self.keyboard.macro_draft.get_mut(index) {
            *action = action.toggled();
            self.keyboard.macro_dirty = true;
            cx.notify();
        }
    }

    /// 调某条动作的延时，步长 10ms，范围 0–5000ms。
    pub(crate) fn kb_macro_delay(&mut self, index: usize, delta: i32, cx: &mut Context<Self>) {
        if let Some(action) = self.keyboard.macro_draft.get_mut(index) {
            let next = action.delay as i32 + delta;
            action.delay = next.clamp(0, 5000) as u16;
            self.keyboard.macro_dirty = true;
            cx.notify();
        }
    }

    pub(crate) fn kb_macro_remove(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.keyboard.macro_draft.len() {
            self.keyboard.macro_draft.remove(index);
            self.keyboard.macro_dirty = true;
            cx.notify();
        }
    }

    pub(crate) fn kb_macro_clear_draft(&mut self, cx: &mut Context<Self>) {
        self.keyboard.macro_draft.clear();
        self.keyboard.macro_dirty = true;
        cx.notify();
    }

    /// 把草稿存进键盘：整表重写。
    pub(crate) fn kb_macro_save(&mut self, cx: &mut Context<Self>) {
        let Some(slot) = self.keyboard.macro_slot else { return };
        if self.keyboard.macro_draft.is_empty() {
            self.kb_macro_delete(cx);
            return;
        }
        let draft = self.keyboard.macro_draft.clone();
        let mut table = self.keyboard.macros.clone();
        table.retain(|entry| entry.id != slot);
        table.push(Macro { id: slot, address: 0, actions: draft });
        self.keyboard.macro_dirty = false;
        self.kb_write(format!("宏 {slot}（{} 个动作）", self.keyboard.macro_draft.len()), cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            kb.set_macros(&table).map_err(|e| e.to_string())
        });
    }

    /// 删掉这个槽的宏（其它槽不动）。
    pub(crate) fn kb_macro_delete(&mut self, cx: &mut Context<Self>) {
        let Some(slot) = self.keyboard.macro_slot else { return };
        let mut table = self.keyboard.macros.clone();
        table.retain(|entry| entry.id != slot);
        self.keyboard.macro_draft.clear();
        self.keyboard.macro_dirty = false;
        self.kb_write(format!("删除宏 {slot}"), cx, move || {
            let kb = Keyboard::open().map_err(|e| e.to_string())?;
            kb.set_macros(&table).map_err(|e| e.to_string())
        });
    }

    // ---- 渲染 ----

    /// 总览页里的键盘卡片，视觉语言与鼠标卡片一致。
    pub(crate) fn render_keyboard_card(&self, cx: &mut Context<Self>, card_w: f32) -> AnyElement {
        let state = &self.keyboard;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;

        let (status_line, status_color) = match (&state.error, state.scanning) {
            (_, true) => ("正在读取…".to_string(), muted),
            (Some(_), _) => ("读取失败 — 点击重试".to_string(), cx.theme().danger),
            (None, false) => {
                let mut parts = vec![state.connection.clone()];
                if state.battery > 0 {
                    parts.push(format!("{}%", state.battery));
                }
                parts.push(format!("固件 v{:.2}", state.firmware));
                (parts.join("  ·  "), muted)
            }
        };

        // 用 6 行小方块画一个键盘剪影，代替鼠标卡片里的产品图。
        let preview_h = (card_w - 66.0).min(300.0).max(120.0) * 0.42;
        let mut silhouette = div().flex().flex_col().gap(px(2.5)).w_full().px_3();
        for row in NUT75_LAYOUT {
            let mut line = div().flex().flex_row().gap(px(2.5)).w_full();
            for slot in *row {
                line = line.child(
                    div()
                        .flex_grow(slot.width)
                        .flex_basis(px(0.))
                        .h(px(9.))
                        .rounded(px(2.))
                        .bg(Hsla { a: 0.22, ..fg }),
                );
            }
            silhouette = silhouette.child(line);
        }

        div()
            .id("kb-card")
            .cursor_pointer()
            .flex()
            .flex_col()
            .gap_1()
            .p_5()
            .rounded_lg()
            .border_1()
            .border_color(Hsla { a: 0.0, ..border })
            .bg(Hsla { a: 0.06, ..fg })
            .hover(|this| this.bg(Hsla { a: 0.10, ..fg }))
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(fg)
                    .truncate()
                    .child("NUT75"),
            )
            .child(div().text_sm().text_color(status_color).child(status_line))
            .child(
                div()
                    .w_full()
                    .h(px(preview_h.max(70.)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(silhouette),
            )
            .on_click(cx.listener(|this, _, _, cx| this.open_keyboard(cx)))
            .into_any_element()
    }

    /// 键盘详情页。
    pub(crate) fn render_keyboard_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.keyboard;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let busy = state.writing.is_some();

        let facts = {
            let mut parts = vec![state.connection.clone()];
            if state.battery > 0 {
                parts.push(format!("{}%", state.battery));
            }
            parts.push(format!("固件 v{:.2}", state.firmware));
            if busy {
                parts.push(format!("正在写入{}…", state.writing.clone().unwrap_or_default()));
            }
            parts.join("  ·  ")
        };

        let header = h_flex()
            .gap_3()
            .items_center()
            .child(
                Button::new("kb-back")
                    .ghost()
                    .icon(IconName::ArrowLeft)
                    .tooltip("返回设备列表")
                    .accessibility_label("返回设备列表")
                    .on_click(cx.listener(|this, _, _, cx| this.close_keyboard(cx))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(fg)
                            .child("NUT75"),
                    )
                    .child(div().text_sm().text_color(muted).child(facts)),
            );

        let tabs = h_flex().gap_2().child(
            h_flex()
                .gap_1()
                .children(KbTab::ALL.into_iter().map(|tab| {
                    self.chip(
                        tab.id().to_string(),
                        tab.label().to_string(),
                        state.tab == tab,
                        cx,
                        move |this, cx| {
                            this.keyboard.tab = tab;
                            cx.notify();
                        },
                    )
                })),
        );

        let body: AnyElement = match state.tab {
            KbTab::Keys => self.render_kb_keys(cx),
            KbTab::Lighting => self.render_kb_lighting(cx),
            KbTab::Macro => self.render_kb_macros(cx),
            KbTab::Advanced => self.render_kb_advanced(cx),
            KbTab::Settings => self.render_kb_settings(cx),
        };

        let mut col = div().flex().flex_col().gap_4().flex_1().min_w_0().max_w(px(900.));
        if let Some(error) = &state.error {
            col = col.child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(cx.theme().danger)
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().danger)
                            .child(format!("读不到键盘：{error}")),
                    )
                    .child(
                        Button::new("kb-retry")
                            .ghost()
                            .compact()
                            .icon(IconName::Redo)
                            .label(if state.scanning { "正在读取…" } else { "重新读取" })
                            .disabled(state.scanning)
                            .on_click(cx.listener(|this, _, _, cx| this.scan_keyboard(cx))),
                    ),
            );
        }
        col = col.child(tabs);
        for note in &state.partial {
            col = col.child(div().text_xs().text_color(cx.theme().danger).child(note.clone()));
        }
        if let Some(error) = &state.write_error {
            col = col.child(div().text_sm().text_color(cx.theme().danger).child(error.clone()));
        }
        col = col.child(body);

        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .gap_4()
            .overflow_y_scrollbar()
            .child(header)
            .child(col)
            .into_any_element()
    }

    /// 区块外壳：标题 + 说明 + 内容。
    fn kb_section(&self, title: &str, hint: Option<&str>, cx: &mut Context<Self>) -> Div {
        let mut card = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground)
                    .child(title.to_string()),
            );
        if let Some(hint) = hint {
            card = card.child(
                div().text_xs().text_color(cx.theme().muted_foreground).child(hint.to_string()),
            );
        }
        card
    }

    /// 改键页：键帽网格 + 选中键的编辑面板。
    fn render_kb_keys(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.keyboard;
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let primary = cx.theme().primary;
        let keys = state.active_keys().to_vec();
        if keys.is_empty() {
            return self.kb_section("键位表", None, cx).child(
                div().text_sm().text_color(muted).child("还没读到键位表，点右上角重新扫描试试。"),
            ).into_any_element();
        }

        let customized = state.customized_count();
        let bound = state.bound_count();
        let layer_row = h_flex()
            .gap_2()
            .items_center()
            .child(self.chip(
                "kb-layer-main".to_string(),
                "主层".to_string(),
                !state.show_fn_layer,
                cx,
                |this, cx| {
                    this.keyboard.show_fn_layer = false;
                    this.keyboard.selected_key = None;
                    cx.notify();
                },
            ))
            .child(self.chip(
                "kb-layer-fn".to_string(),
                "FN 层".to_string(),
                state.show_fn_layer,
                cx,
                |this, cx| {
                    this.keyboard.show_fn_layer = true;
                    this.keyboard.selected_key = None;
                    cx.notify();
                },
            ))
            .child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child(format!(
                        "{bound} 个位置有绑定，其中 {customized} 个改成了非键盘功能"
                    )),
            );

        let mut board = div().flex().flex_col().gap_1().w_full();
        for row in NUT75_LAYOUT {
            let mut line = div().flex().flex_row().gap_1().w_full();
            for slot in *row {
                let binding = keys.get(slot.index).copied();
                let is_default = binding.map(|b| b.is_default()).unwrap_or(true);
                let selected = state.selected_key == Some(slot.index);
                let secondary = binding.and_then(|b| {
                    // 键位被改过时，把新功能写在键帽下面
                    match b.page() {
                        KeyPage::Keyboard | KeyPage::Default => None,
                        other => Some(format!("{}", other.label())),
                    }
                });
                let index = slot.index;
                line = line.child(
                    div()
                        .id(SharedString::from(format!("kb-key-{index}")))
                        .cursor_pointer()
                        .flex_grow(slot.width)
                        .flex_basis(px(0.))
                        .h(px(40.))
                        .px_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_0p5()
                        .rounded_md()
                        .border_1()
                        .border_color(if selected { primary } else { cx.theme().border })
                        .bg(if selected {
                            Hsla { a: 0.18, ..primary }
                        } else if is_default {
                            Hsla { a: 0.06, ..fg }
                        } else {
                            Hsla { a: 0.02, ..primary }
                        })
                        .child(
                            div()
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(if is_default { fg } else { primary })
                                .truncate()
                                .child(slot.label.to_string()),
                        )
                        .children(secondary.map(|text| {
                            div().text_xs().text_color(muted).truncate().child(text)
                        }))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.keyboard.selected_key =
                                if this.keyboard.selected_key == Some(index) {
                                    None
                                } else {
                                    Some(index)
                                };
                            cx.notify();
                        })),
                );
            }
            board = board.child(line);
        }

        let mut col = div().flex().flex_col().gap_3();
        col = col.child(layer_row);
        col = col.child(board);

        if let Some(index) = state.selected_key {
            col = col.child(self.render_kb_key_editor(index, &keys, cx));
        } else {
            col = col.child(
                div()
                    .text_xs()
                    .text_color(muted)
                    .child("点一个键帽可以看到它现在的绑定，并改成别的功能。"),
            );
        }

        self.kb_section("键位表", None, cx).child(col).into_any_element()
    }

    /// 选中键的编辑面板。
    fn render_kb_key_editor(
        &self,
        index: usize,
        keys: &[KeyBinding],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let fg = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let border = cx.theme().border;
        let primary = cx.theme().primary;
        let Some(binding) = keys.get(index).copied() else { return div().into_any_element() };
        let name = NUT75_LAYOUT
            .iter()
            .flat_map(|row| row.iter())
            .find(|slot| slot.index == index)
            .map(|slot| slot.label.to_string())
            .unwrap_or_else(|| format!("键位 {index}"));
        let busy = self.keyboard.writing.is_some();

        let reset = Button::new("kb-key-reset")
            .ghost()
            .label("恢复出厂默认")
            .disabled(busy || binding.is_default())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.kb_assign_key(index, None, cx);
            }));

        // 从布局里收集"这个键位号对应哪个 USB usage"，作为常用键的来源。
        let mut hints = div().flex().flex_wrap().gap_1().w_full();
        for row in NUT75_LAYOUT {
            for slot in *row {
                let Some(other) = keys.get(slot.index) else { continue };
                let Some(usage) = other.keyboard_usage() else { continue };
                let selected = binding.keyboard_usage() == Some(usage);
                let label = slot.label.to_string();
                hints = hints.child(self.chip(
                    format!("kb-pick-{usage}"),
                    label,
                    selected,
                    cx,
                    move |this, cx| this.kb_assign_key(index, Some(keymap::keyboard_key(usage)), cx),
                ));
            }
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(border)
            .child(
                h_flex()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_0p5()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(fg)
                                    .child(format!("{name}（键位 {index}）")),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(muted)
                                    .child(format!(
                                        "当前：{}  ·  原始字节 {:02X} {:02X} {:02X} {:02X}",
                                        binding.describe(),
                                        binding.0[0],
                                        binding.0[1],
                                        binding.0[2],
                                        binding.0[3]
                                    )),
                            ),
                    )
                    .child(reset),
            )
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("改成键盘按键"),
            )
            .child(hints)
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("改成多媒体 / 鼠标"),
            )
            .child({
                // 两批候选分开建，避免两个闭包同时借用 cx。
                let mut row = h_flex().flex_wrap().gap_1();
                for (label, usage) in MEDIA_KEYS {
                    let usage = *usage;
                    row = row.child(self.chip(
                        format!("kb-media-{usage}"),
                        (*label).to_string(),
                        binding.consumer_usage() == Some(usage),
                        cx,
                        move |this, cx| {
                            this.kb_assign_key(index, Some(keymap::consumer_key(usage)), cx)
                        },
                    ));
                }
                for (label, button) in MOUSE_KEYS {
                    let button = *button;
                    row = row.child(self.chip(
                        format!("kb-mouse-{button}"),
                        (*label).to_string(),
                        binding.mouse_button() == Some(button),
                        cx,
                        move |this, cx| {
                            this.kb_assign_key(index, Some(keymap::mouse_key(button)), cx)
                        },
                    ));
                }
                row
            })
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(muted)
                    .child("改成宏（宏在「宏」标签页里编辑）"),
            )
            .child({
                let mut row = h_flex().flex_wrap().gap_1();
                let mut has_macro = false;
                for entry in &self.keyboard.macros {
                    if entry.actions.is_empty() {
                        continue;
                    }
                    has_macro = true;
                    let slot = entry.id;
                    row = row.child(self.chip(
                        format!("kb-to-macro-{slot}"),
                        format!("宏 {}", slot + 1),
                        binding.macro_index() == Some(slot),
                        cx,
                        move |this, cx| {
                            this.kb_assign_key(index, Some(keymap::macro_key(slot)), cx)
                        },
                    ));
                }
                if !has_macro {
                    row = row.child(
                        div()
                            .text_xs()
                            .text_color(muted)
                            .child("键盘里还没有宏，先去「宏」页建一个。"),
                    );
                }
                row
            })
            .child(
                div()
                    .text_xs()
                    .text_color(Hsla { a: 0.8, ..primary })
                    .child("改动会立刻写入键盘。写之前的位置想找回，按「恢复出厂默认」即可回到出厂的按键。"),
            )
            .into_any_element()
    }

    /// 灯效页。
    ///
    /// 模式现在可以切了：名字和编号的对照表来自官方驱动前端的
    /// `lightingModeList`（NUT75 是 12 个基础模式 + 23/24/25 三个追加模式），
    /// 所以不再是"只知道编号"的状态。每个模式吃哪些参数也跟着官方那套开关走
    /// ——静态恒亮没有速度，流光不吃主色，照模式显示才不会给用户一堆没用的
    /// 控件。
    fn render_kb_lighting(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let Some(led) = self.keyboard.led else {
            return self
                .kb_section("灯效", None, cx)
                .child(div().text_sm().text_color(muted).child("还没读到灯效数据。"))
                .into_any_element();
        };
        let busy = self.keyboard.writing.is_some();
        let mode = effect_mode(led.mode);
        let mode_name = effect_mode_name(led.mode);

        let modes = div().flex().flex_wrap().gap_1().children(EFFECT_MODES.iter().map(|item| {
            let id = item.id;
            self.chip(
                format!("kb-mode-{id}"),
                item.name.to_string(),
                led.mode == id,
                cx,
                move |this, cx| {
                    this.kb_edit_lighting(format!("灯效 {}", item.name), move |e| e.mode = id, cx)
                },
            )
        }));

        let brightness = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .children((1u8..=6).map(|level| {
                self.chip(
                    format!("kb-bright-{level}"),
                    format!("{level}"),
                    led.brightness == level,
                    cx,
                    move |this, cx| {
                        this.kb_edit_lighting(format!("亮度 {level}"), move |e| e.brightness = level, cx)
                    },
                )
            }));
        let speed = div().flex().flex_wrap().gap_1().children((1u8..=6).map(|level| {
            self.chip(
                format!("kb-speed-{level}"),
                format!("{level}"),
                led.speed == level,
                cx,
                move |this, cx| {
                    this.kb_edit_lighting(format!("速度 {level}"), move |e| e.speed = level, cx)
                },
            )
        }));
        let colors = div().flex().flex_wrap().gap_1().children(LIGHT_COLORS.iter().map(|(name, rgb)| {
            let rgb = *rgb;
            let selected = (led.red, led.green, led.blue) == rgb;
            let label = (*name).to_string();
            self.chip(
                format!("kb-color-{name}"),
                label,
                selected,
                cx,
                move |this, cx| {
                    this.kb_edit_lighting(
                        "颜色",
                        move |e| {
                            e.red = rgb.0;
                            e.green = rgb.1;
                            e.blue = rgb.2;
                        },
                        cx,
                    )
                },
            )
        }));

        let mut col = self.kb_section(
            "灯效模式",
            Some("15 个模式取自官方驱动的模式表。有的模式不吃颜色、有的没有速度，这里按模式显示对应参数。"),
            cx,
        );
        col = col.child(modes);
        col = col.child(
            div()
                .text_xs()
                .text_color(muted)
                .child(format!("当前：{mode_name}（编号 {}）", led.mode)),
        );

        if mode.map(|m| m.show_speed).unwrap_or(true) {
            col = col.child(div().text_xs().text_color(muted).child("速度（1–6 档）")).child(speed);
        }
        if mode.map(|m| m.show_color).unwrap_or(true) {
            col = col.child(div().text_xs().text_color(muted).child("主色")).child(colors);
        } else {
            col = col.child(div().text_xs().text_color(muted).child("这个模式按固定色彩跑，不吃主色。"));
        }
        col = col
            .child(div().text_xs().text_color(muted).child("亮度（1–6 档）"))
            .child(brightness);
        if mode.map(|m| m.show_direction).unwrap_or(false) {
            col = col.child(div().text_xs().text_color(muted).child(
                "这个模式还能调方向，但方向字节的取值没验证过，暂不开放（在官方驱动里调好再回来即可）。",
            ));
        }
        if busy {
            col = col.child(div().text_xs().text_color(muted).child("正在写入…"));
        }
        col.child(div().text_xs().text_color(muted).child(format!(
            "原始 16 字节：{}",
            led.raw().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
        )))
        .into_any_element()
    }

    /// 宏页：槽位网格 + 单个宏的动作编辑。
    ///
    /// 动作列表是「延时 + 键码 + 按下/抬起」的平铺，和协议里的 4 字节结构
    /// 一一对应，不做任何抽象。这样界面上看到什么，写进设备的就是什么。
    fn render_kb_macros(&self, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.keyboard;
        let muted = cx.theme().muted_foreground;
        let fg = cx.theme().foreground;
        let border = cx.theme().border;
        let primary = cx.theme().primary;
        let busy = state.writing.is_some();
        let keys = &state.keys;

        // 槽位网格：10 列 × 10 行，有内容的槽点亮。
        let mut grid = div().flex().flex_wrap().gap_1().w_full();
        for slot in 0..100u8 {
            let saved = state.macros.iter().find(|entry| entry.id == slot);
            let selected = state.macro_slot == Some(slot);
            let count = saved.map(|entry| entry.actions.len()).unwrap_or(0);
            let id = format!("kb-macro-slot-{slot}");
            grid = grid.child(
                div()
                    .id(SharedString::from(id))
                    .cursor_pointer()
                    .w(px(38.))
                    .h(px(30.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .rounded_md()
                    .border_1()
                    .border_color(if selected { primary } else { border })
                    .bg(if selected {
                        Hsla { a: 0.18, ..primary }
                    } else if count > 0 {
                        Hsla { a: 0.04, ..primary }
                    } else {
                        Hsla { a: 0.04, ..fg }
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(if count > 0 { FontWeight::SEMIBOLD } else { FontWeight::NORMAL })
                            .text_color(if count > 0 { primary } else { muted })
                            .child(format!("{slot}")),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.kb_pick_macro_slot(slot, cx))),
            );
        }

        let occupied = state.macros.iter().filter(|entry| !entry.actions.is_empty()).count();
        let col = self.kb_section(
            "宏槽位",
            Some("键盘里一共 100 个槽（0–99）。有内容的槽是彩色的，点一个槽开始编辑。"),
            cx,
        )
        .child(div().text_xs().text_color(muted).child(format!(
            "已用 {occupied} 个槽  ·  当前编辑：{}",
            state.macro_slot.map(|s| format!("槽 {s}")).unwrap_or_else(|| "未选择".to_string())
        )))
        .child(grid);

        let Some(slot) = state.macro_slot else {
            return col
                .child(div().text_xs().text_color(muted).child("选一个槽，然后往里面加动作。"))
                .into_any_element();
        };

        // ---- 动作列表 ----
        let draft = &state.macro_draft;
        let names = key_names(keys);
        let mut actions = div().flex().flex_col().gap_1().w_full();
        if draft.is_empty() {
            actions = actions.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("这个槽还是空的。下面点键名就能往里面加「按下 + 抬起」。"),
            );
        }
        for (index, action) in draft.iter().enumerate() {
            let name = action_name(action, &names);
            let delay = action.delay;
            let press = action.is_press();
            actions = actions.child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .border_1()
                    .border_color(border)
                    .child(div().text_xs().text_color(muted).w(px(26.)).child(format!("{index}")))
                    .child(
                        self.chip(
                            format!("kb-macro-press-{index}"),
                            if press { "按下".to_string() } else { "抬起".to_string() },
                            press,
                            cx,
                            move |this, cx| this.kb_macro_toggle(index, cx),
                        ),
                    )
                    .child(div().text_sm().text_color(fg).w(px(90.)).truncate().child(name))
                    .child(
                        Button::new(SharedString::from(format!("kb-macro-minus-{index}")))
                            .ghost()
                            .compact()
                            .label("−")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| this.kb_macro_delay(index, -10, cx))),
                    )
                    .child(div().text_xs().text_color(muted).child(format!("{delay} ms")))
                    .child(
                        Button::new(SharedString::from(format!("kb-macro-plus-{index}")))
                            .ghost()
                            .compact()
                            .label("+")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| this.kb_macro_delay(index, 10, cx))),
                    )
                    .child(
                        Button::new(SharedString::from(format!("kb-macro-del-{index}")))
                            .ghost()
                            .compact()
                            .label("删除")
                            .disabled(busy)
                            .on_click(cx.listener(move |this, _, _, cx| this.kb_macro_remove(index, cx))),
                    ),
            );
        }

        // ---- 追加候选 ----
        let mut key_row = div().flex().flex_wrap().gap_1().w_full();
        for (usage, name) in &names {
            let usage = *usage;
            key_row = key_row.child(self.chip(
                format!("kb-macro-key-{usage}"),
                (*name).to_string(),
                false,
                cx,
                move |this, cx| this.kb_macro_append_key(usage, cx),
            ));
        }
        let mut mouse_row = h_flex().flex_wrap().gap_1();
        for (name, button) in MOUSE_KEYS {
            let button = *button;
            mouse_row = mouse_row.child(self.chip(
                format!("kb-macro-mouse-{button}"),
                (*name).to_string(),
                false,
                cx,
                move |this, cx| this.kb_macro_append_mouse(button, cx),
            ));
        }

        let save = Button::new("kb-macro-save")
            .primary()
            .label(if busy { "正在写入…" } else { "保存到键盘" })
            .disabled(busy || !state.macro_dirty)
            .on_click(cx.listener(|this, _, _, cx| this.kb_macro_save(cx)));
        let remove = Button::new("kb-macro-delete")
            .ghost()
            .label("删除这个宏")
            .disabled(busy || (draft.is_empty() && !state.macros.iter().any(|e| e.id == slot)))
            .on_click(cx.listener(|this, _, _, cx| this.kb_macro_delete(cx)));
        let clear = Button::new("kb-macro-clear")
            .ghost()
            .label("清空草稿")
            .disabled(busy || draft.is_empty())
            .on_click(cx.listener(|this, _, _, cx| this.kb_macro_clear_draft(cx)));

        col.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_4()
                .rounded_lg()
                .border_1()
                .border_color(border)
                .child(
                    h_flex()
                        .justify_between()
                        .items_center()
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(fg)
                                        .child(format!("槽 {slot}  ·  {} 个动作", draft.len())),
                                )
                                .child(div().text_xs().text_color(muted).child(
                                    "延时是这条动作之前等待的毫秒数。改完点「保存到键盘」才生效。",
                                )),
                        )
                        .child(
                            h_flex()
                                .gap_1()
                                .child(clear)
                                .child(remove)
                                .child(save),
                        ),
                )
                .children(if state.macro_dirty {
                    Some(div().text_xs().text_color(Hsla { a: 0.9, ..primary }).child("有未保存的改动"))
                } else {
                    None
                })
                .child(actions)
                .child(div().text_xs().text_color(muted).child("追加键盘键"))
                .child(key_row)
                .child(div().text_xs().text_color(muted).child("追加鼠标键"))
                .child(mouse_row),
        )
        .into_any_element()
    }

    /// 高级键页。
    fn render_kb_advanced(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let keys = self.keyboard.active_keys();
        // 高级键没有独立命令，全都编码在键位表的 4 字节里，所以在这里从
        // 键位表里反查，而不是另发一条命令。
        let mut found = Vec::new();
        for (index, binding) in keys.iter().enumerate() {
            let page = binding.page();
            if matches!(
                page,
                KeyPage::Socd | KeyPage::Mt | KeyPage::Tgl | KeyPage::Cb | KeyPage::Dks | KeyPage::Mpt
            ) {
                found.push((index, page, *binding));
            }
        }

        let mut col = self.kb_section(
            "高级键",
            Some("SOCD / MT / TGL / CB 没有独立命令，都编码在键位表的 4 个字节里，写入走的还是键位表。"),
            cx,
        );
        if found.is_empty() {
            col = col.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child("当前没有任何键位使用了高级键功能。"),
            );
        } else {
            for (index, page, binding) in found {
                col = col.child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .text_sm()
                        .child(format!("键位 {index}"))
                        .child(page.label())
                        .child(format!(
                            "{:02X} {:02X} {:02X} {:02X}",
                            binding.0[0], binding.0[1], binding.0[2], binding.0[3]
                        )),
                );
            }
        }
        col.into_any_element()
    }

    /// 设置页。
    fn render_kb_settings(&self, cx: &mut Context<Self>) -> AnyElement {
        let muted = cx.theme().muted_foreground;
        let Some(settings) = self.keyboard.settings else {
            return self
                .kb_section("设置", None, cx)
                .child(div().text_sm().text_color(muted).child("还没读到设置数据。"))
                .into_any_element();
        };

        let delay = div().flex().flex_wrap().gap_1().children((1u8..=5).map(|level| {
            self.chip(
                format!("kb-delay-{level}"),
                format!("{level}"),
                settings.key_delay == level,
                cx,
                move |this, cx| {
                    this.kb_edit_settings(format!("按键延时 {level}"), move |s| s.key_delay = level, cx)
                },
            )
        }));

        let sleep = div().flex().flex_wrap().gap_1().children(
            SLEEP_STEPS.iter().map(|(label, value)| {
                let value = *value;
                self.chip(
                    format!("kb-sleep-{value}"),
                    (*label).to_string(),
                    settings.sleep_time == value,
                    cx,
                    move |this, cx| {
                        this.kb_edit_settings(format!("休眠 {label}"), move |s| s.sleep_time = value, cx)
                    },
                )
            }),
        );

        let rate = settings
            .report_rate_hz_for_nut75()
            .map(|hz| format!("{hz} Hz"))
            .unwrap_or_else(|| format!("未知（码值 {}）", settings.report_rate));

        let is_wireless = self.keyboard.connection.contains("2.4G");
        let active_rate = settings.active_report_rate_hz();
        let rates = div().flex().flex_wrap().gap_1().children(
            GameMode::REPORT_RATES.iter().map(|hz| {
                let hz = *hz;
                self.chip(
                    format!("kb-rate-{hz}"),
                    format!("{hz} Hz"),
                    active_rate == Some(hz),
                    cx,
                    move |this, cx| {
                        this.kb_set_report_rate(hz, cx);
                    },
                )
            }),
        );

        let nkro_row = h_flex().justify_between().items_center().child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(div().text_sm().child("NKRO 全键无冲"))
                .child(div().text_xs().text_color(muted).child("同时按下任意多键都能全部识别。")),
        );
        let nkro = self.keyboard.settings.map(|s| s.nkro_switch == 1).unwrap_or(false);

        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                self.kb_section(
                    "回报率",
                    Some(if is_wireless {
                        "当前是 2.4G 连接。改完如果键盘短暂掉线，按一下键或等几秒会自动重连。"
                    } else {
                        "有线连接改的是 reportRate 档位码。"
                    }),
                    cx,
                )
                .child(div().text_sm().child(format!("当前 {}（码值 {}）", rate, settings.report_rate)))
                .child(rates)
                .child(div().text_xs().text_color(muted).child(format!(
                    "2.4G 专用字段 wireless_report_rate = {}（非 0 时以它为准）",
                    settings.wireless_report_rate
                ))),
            )
            .child(
                self.kb_section("按键延时", Some("数值越小越灵敏，但更容易误触。"), cx)
                    .child(delay),
            )
            .child(
                self.kb_section("休眠", Some("闲置多久后进入省电。"), cx).child(sleep),
            )
            .child(
                self.kb_section("其它", None, cx)
                    .child(nkro_row.child(Switch::new("kb-nkro").checked(nkro).on_change(
                        cx.listener(|this, on: &bool, _, cx| {
                            let value = if *on { 1 } else { 0 };
                            this.kb_edit_settings("NKRO", move |s| s.nkro_switch = value, cx);
                        }),
                    ))),
            )
            .into_any_element()
    }
}

/// 键位表里出现过的键盘 usage → 键名。
///
/// 名字直接用布局里的键帽标签（`NUT75_LAYOUT` 的 `label`），所以宏里显示的是
/// "A"、"Spacebar" 这种键面上真有的字，而不是裸 usage。同一个 usage 只留一条。
fn key_names(keys: &[KeyBinding]) -> Vec<(u8, &'static str)> {
    let mut out: Vec<(u8, &'static str)> = Vec::new();
    for row in NUT75_LAYOUT {
        for slot in *row {
            let Some(binding) = keys.get(slot.index) else { continue };
            let Some(usage) = binding.keyboard_usage() else { continue };
            if !out.iter().any(|(existing, _)| *existing == usage) {
                out.push((usage, slot.label));
            }
        }
    }
    out.sort_by_key(|(usage, _)| *usage);
    out
}

/// 一条宏动作显示成什么。
fn action_name(action: &Action, names: &[(u8, &'static str)]) -> String {
    if action.is_mouse() {
        return MOUSE_KEYS
            .iter()
            .find(|(_, button)| *button == action.key_code)
            .map(|(label, _)| format!("鼠标·{label}"))
            .unwrap_or_else(|| format!("鼠标键 {}", action.key_code));
    }
    names
        .iter()
        .find(|(usage, _)| *usage == action.key_code)
        .map(|(_, label)| (*label).to_string())
        .unwrap_or_else(|| format!("键码 0x{:02X}", action.key_code))
}

/// 改键页里的多媒体键候选。
const MEDIA_KEYS: &[(&str, u16)] = &[
    ("音量 +", 0x00E9),
    ("音量 −", 0x00EA),
    ("静音", 0x00E2),
    ("播放/暂停", 0x00CD),
    ("上一曲", 0x00B6),
    ("下一曲", 0x00B5),
    ("停止", 0x00B7),
    ("计算器", 0x0192),
    ("资源管理器", 0x0194),
    ("浏览器主页", 0x0223),
];

/// 改键页里的鼠标键候选。
const MOUSE_KEYS: &[(&str, u8)] = &[
    ("左键", 1),
    ("右键", 2),
    ("中键", 3),
    ("后退", 4),
    ("前进", 5),
];

/// 灯效页的颜色候选。
const LIGHT_COLORS: &[(&str, (u8, u8, u8))] = &[
    ("白", (255, 255, 255)),
    ("红", (255, 0, 0)),
    ("橙", (255, 106, 0)),
    ("黄", (255, 200, 0)),
    ("绿", (0, 255, 64)),
    ("青", (0, 255, 208)),
    ("蓝", (0, 64, 255)),
    ("紫", (106, 0, 255)),
    ("品红", (255, 0, 160)),
];

/// 休眠档位。0 = 不休眠。
const SLEEP_STEPS: &[(&str, u8)] = &[("关闭", 0), ("1 分钟", 1), ("10 分钟", 10), ("20 分钟", 20), ("30 分钟", 30)];
