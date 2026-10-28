// What the device page's Buttons, Lighting and Advanced tabs offer for a
// connected device, and how each control writes. A transliteration of
// src/components/Device{Buttons,Lighting,Advanced}Tab.tsx: the same brand
// gating on the same MouseStatus fields, the same write functions — but as
// plain data, so the Rust side can draw any brand without knowing it.

import { RAZER_BUTTON_CONTROLS, RAZER_BUTTON_CONTROL_LABEL, RAZER_BUTTON_MAPPINGS, RAZER_LOCKED_BUTTON_CONTROL, RAZER_TOGGLE_CONTROLS, RAZER_TOGGLE_CONTROL_INFO } from "@openmouse/protocol/razer";
import { EGG_BUTTON_NAMES, EGG_BUTTON_MAPPINGS } from "@openmouse/protocol/drivers/endgame/egg-op1-hid";
import { TEEVOLUTION_PERFORMANCE_TIME_LABELS } from "@openmouse/protocol/teevolution";
import * as w from "./native-hid/write";
import { withLogitechClient } from "./native-hid/logitech-actions";
import { callClientMethod } from "./generic-write";
import type { HidInterfaceInfo } from "./native-hid/tauri-hid-device";
import type { DeviceCapabilities } from "./native-hid/write";

type Option = { value: string | number; label: string };
type Control =
  | { id: string; label: string; description?: string; kind: "choice"; options: Option[]; current: string | number | null }
  | { id: string; label: string; description?: string; kind: "stepped"; options: Option[]; current: string | number | null }
  | { id: string; label: string; description?: string; kind: "toggle"; current: boolean }
  | { id: string; label: string; description?: string; kind: "number"; min: number; max: number; step?: number; decimals?: number; unit?: string; zeroLabel?: string; current: number }
  | { id: string; label: string; description?: string; kind: "info"; current: string };
type Section = { title: string; controls: Control[] };

/** App preferences the layout depends on, pushed in by the host (`setPrefs`). */
let linkLeftRight: boolean | null = null;
export function setPrefs(prefs: { linkLeftRight?: boolean | null }) {
  linkLeftRight = typeof prefs.linkLeftRight === "boolean" ? prefs.linkLeftRight : null;
}

const opts = (values: readonly (string | number)[]): Option[] => values.map((v) => ({ value: v, label: String(v) }));

/**
 * 界面文案的中英映射。未收录的条目原样返回，所以驱动新增文案不会因此出错。
 * 只翻译显示用的 title / label / description / options[].label / zeroLabel，
 * 不翻译 current：choice 与 stepped 的 current 既是提交值也是选中态判断依据。
 */
const ZH: Record<string, string> = {
  // ── 通用 / 分组标题 ──
  "More settings": "更多设置",
  "Buttons": "按键",
  "Profiles": "配置",
  "Active profile": "当前配置",
  "Pairing": "配对",
  "Switch": "开关",
  "Basic": "基础",
  "Normal": "标准",
  "Off": "关闭",
  "On": "开启",
  "Default": "默认",
  "Disabled": "已禁用",
  "Enabled": "已启用",
  "Stop": "停止",
  "Start pairing": "开始配对",
  "Early": "提前",
  "Late": "延后",
  "Optical": "光学",
  "Mechanical": "机械",
  "Magnetic": "磁微动",
  "Device info": "设备信息",
  "Battery": "电量",
  "Battery status": "电量状态",
  "Battery indicator": "电量指示",
  "Battery warning": "低电量提醒",
  "Low battery": "低电量",
  "Dongle": "接收器",
  "Dongle LED": "接收器指示灯",
  "Dongle light": "接收器指示灯",
  "Receiver": "接收器",
  "Receiver LED": "接收器指示灯",
  "Receiver light": "接收器指示灯",
  "Receiver ID": "接收器 ID",
  "Receiver firmware": "接收器固件",
  "Omni receiver": "全能接收器",

  // ── 性能 / 传感器 ──
  "Polling rate": "回报率",
  "Button polling rate": "按键回报率",
  "Connection & polling rate": "连接与回报率",
  "DPI stages": "DPI 档位",
  "DPI acceleration": "DPI 加速",
  "DPI indicator effect": "DPI 指示灯效果",
  "DPI light sleep": "DPI 指示灯休眠",
  "CPI levels": "CPI 档位",
  "Separate DPI axes": "独立 DPI 轴",
  "Sensor mode": "传感器模式",
  "Sensor scanning": "传感器扫描",
  "Motion sync": "运动同步",
  "Motion-jitter filter": "运动抖动过滤",
  "Angle snapping": "角度吸附",
  "Angle tuning": "角度微调",
  "Acceleration curve": "加速曲线",
  "Ripple control": "纹波控制",
  "Wheel acceleration": "滚轮加速",
  "Wheel guard": "滚轮防护",
  "Wheel guard window": "滚轮防护窗口",
  "Wheel mode": "滚轮模式",
  "Glass mode": "玻璃模式",
  "PAW lift-off": "PAW 抬起高度",
  "Slamclick filter": "防撞点击过滤",
  "Slam-Click": "Slam-Click",
  "HyperClick": "HyperClick",
  "Hyper mode": "极速模式",
  "Turbo mode": "加速模式",
  "Power mode": "电源模式",
  "Low power mode": "低功耗模式",
  "Performance / hyper mode": "性能 / 极速模式",
  "Performance duration": "性能持续时间",
  "USB speed": "USB 速度",
  "Long-range mode": "远距离模式",
  "Debounce": "去抖时间",
  "Sleep timeout": "休眠时间",
  "8K polling divider": "8K 回报率分频",
  "Perfect Polling": "Perfect Polling",

  // ── 按键 / 触感 ──
  "Actuation point": "触发行程",
  "Release point": "释放行程",
  "Trigger point": "触发点",
  "Rapid trigger": "快速触发",
  "Rapid trigger sensitivity": "快速触发灵敏度",
  "Lightning trigger": "闪电触发",
  "Hall-effect buttons": "霍尔效应按键",
  "Lightforce switches": "Lightforce 微动",
  "LightForce switch mode": "LightForce 微动模式",
  "TMR analog": "TMR 模拟",
  "Anti-mistouch": "防误触",
  "Click haptics": "点击触感",
  "Haptic feedback": "触感反馈",
  "Haptic strength": "触感强度",
  "Haptics battery saving": "触感省电",
  "Primary button": "主按键",
  "Bottom button": "底部按键",
  "Fire key": "开火键",
  "Button combinations": "按键组合",
  "Clicks per press": "每次按下点击数",
  "Interval": "间隔",
  "Scroll wheel": "滚轮",
  "Scroll direction": "滚动方向",
  "Invert scroll direction": "反转滚动方向",
  "Invert thumb wheel": "反转侧滚轮",
  "Tournament scroll": "比赛模式滚动",
  "Passthrough window": "透传窗口",
  "High-resolution scrolling": "高精度滚动",
  "SmartShift sensitivity": "SmartShift 灵敏度",
  "System mode": "系统模式",
  "Optical Engine": "光学引擎",
  "Calibration reference": "校准参考",
  "Separate DPI axes ": "独立 DPI 轴",

  // ── 说明文字 ──
  "Choose what this button sends.": "选择此按键发送的动作。",
  "Change both buttons together.": "同时更改两个按键。",
  "Milliseconds.": "毫秒。",
  "Milliseconds between clicks.": "两次点击之间的毫秒数。",
  "Higher means a deeper press.": "数值越大，按下行程越深。",
  "How far the button travels before it clicks. Lower is lighter.": "按键触发前的行程。数值越小手感越轻。",
  "How far the button travels back before it releases.": "按键回弹多远后释放。",
  "How far the button must lift to reset. Off at 0.": "按键抬起多高后重置。0 表示关闭。",
  "Re-clicks the moment the button moves back up.": "按键回弹的瞬间立即重新触发。",
  "How long the mouse stays awake when idle before sleeping.": "鼠标空闲多久后进入休眠。",
  "Slows the mouse down to save battery below this level.": "电量低于此值时降低性能以省电。",
  "Synchronizes X/Y sensor sampling for smoother diagonals.": "同步 X/Y 轴采样，让对角线移动更平滑。",
  "Filters sensor ripple at high polling rates for more stable tracking.": "在高回报率下过滤传感器抖动，让追踪更稳定。",
  "Pins the sensor to its highest frame rate.": "将传感器固定在最高帧率。",
  "Runs the sensor at its full performance setting.": "让传感器以满性能运行。",
  "Runs the sensor at its highest performance setting.": "让传感器以最高性能运行。",
  "Rounds movement to straight lines and diagonals for a steadier cursor.": "将移动吸附到直线与对角线，让光标更稳定。",
  "Rotates the sensor to correct for how you hold the mouse. 0° is off.": "旋转传感器以校正握持角度。0° 表示关闭。",
  "Tracking tuned for glass surfaces.": "针对玻璃表面优化的追踪。",
  "Trades battery for link range on receivers that offer it.": "在支持的接收器上，以续航换取连接距离。",
  "Trades battery life against performance.": "在续航与性能之间取舍。",
  "Full pins polling to 1000 Hz; High enables faster rates.": "全速将回报率锁定在 1000 Hz；High 可启用更高回报率。",
  "Free-spinning or ratcheted, as the button behind the wheel toggles.": "自由滚动或分段滚动，由滚轮后方的按键切换。",
  "How gentle a flick releases the ratchet. 255 turns SmartShift off.": "多轻的拨动即可释放棘轮。255 表示关闭 SmartShift。",
  "Hybrid uses an optical and a mechanical stage; Optical is purely optical.": "混合模式使用光学加机械两级；光学模式为纯光学触发。",
  "Ignored-click window in ms; 0 is off.": "忽略点击的时间窗（毫秒）；0 表示关闭。",
  "Seconds before the DPI indicator turns off, separate from mouse sleep.": "DPI 指示灯熄灭前的秒数，与鼠标休眠独立。",
  "On-device sensitivity stages; pick the active one.": "设备内置的灵敏度档位；选择当前生效的一档。",
  "Open the receiver for a new device.": "打开接收器以连接新设备。",
  "The receiver's status light.": "接收器的状态指示灯。",
  "What the 2.4 GHz receiver's light shows.": "2.4 GHz 接收器指示灯的显示内容。",
  "Switches the mouse's stored profile.": "切换鼠标的板载配置。",
  "Lets button chords change mouse settings without the driver.": "让按键组合在没有驱动时也能更改鼠标设置。",
  "Which button acts as the primary click.": "哪个按键作为主点击。",
  "Strength of this button's click feedback.": "此按键点击反馈的强度。",
  "Magnetic needs a calibrated mouse.": "磁微动需要先校准鼠标。",
  "Can only be changed with the mouse on its cable.": "仅在鼠标插线时可更改。",
  "Needs mouse firmware 57 or later.": "需要鼠标固件 57 或更高版本。",
  "Set up with a macro or custom binding — not editable here.": "已通过宏或自定义绑定设置 — 此处不可编辑。",
  "Solid white": "常亮白色",
  "Battery indicator effect": "电量指示效果",

  // ── 分组标题（品牌前缀保留）──
  "Razer — Buttons": "Razer — 按键",
  "Razer — Scroll & toggles": "Razer — 滚轮与开关",
  "Endgame Gear — Buttons": "Endgame Gear — 按键",
  "Endgame Gear — Debounce": "Endgame Gear — 去抖",
  "GX switch — left button": "GX 微动 — 左键",
  "GX switch — right button": "GX 微动 — 右键",
  "Starlight X · Lift-off": "Starlight X · 抬起高度",

  // ── 按键映射选项 ──
  "Left Click": "左键单击",
  "Left click": "左键单击",
  "Right Click": "右键单击",
  "Right click": "右键单击",
  "Middle Click": "中键单击",
  "Middle click": "中键单击",
  "Left button": "左键",
  "Right button": "右键",
  "Middle button": "中键",
  "Back": "后退",
  "Back button": "后退键",
  "Backward": "后退",
  "Forward": "前进",
  "Forward button": "前进键",
  "Scroll Up": "向上滚动",
  "Scroll Up ": "向上滚动",
  "Scroll up": "向上滚动",
  "Scroll Down": "向下滚动",
  "Scroll down": "向下滚动",
  "Wheel Up": "滚轮上滚",
  "Wheel Down": "滚轮下滚",
  "Volume Up": "音量 +",
  "Volume +": "音量 +",
  "Volume Down": "音量 -",
  "Volume −": "音量 -",
  "Mute": "静音",
  "Play / Pause": "播放 / 暂停",
  "Play/Pause": "播放/暂停",
  "Next Track": "下一曲",
  "Next track": "下一曲",
  "Previous Track": "上一曲",
  "Prev track": "上一曲",
  "Browser Home": "浏览器主页",
  "File Explorer": "文件资源管理器",
  "Keyboard shortcut": "键盘快捷键",
  "Sensitivity Button": "灵敏度按键",
  "CPI Loop": "CPI 循环",
  "DPI loop": "DPI 循环",
  "Fixed CPI": "固定 CPI",
  "gaming surface mode": "游戏表面模式",

  // ── 设备信息 / 状态 ──
  "Unit ID": "设备 ID",
  "Model ID": "型号 ID",
  "Device name": "设备名称",
  "Connection": "连接方式",
  "Transports": "传输方式",
  "Signal strength": "信号强度",
  "Battery voltage": "电池电压",
  "Easy-Switch": "Easy-Switch",
  "Layer": "图层",
  "Wireless": "无线",
  "Wired": "有线",
  "Connected": "已连接",
  "Not connected": "未连接",
  "Full": "满电",
  "Steady": "常亮",
  "Always on": "常亮",
  "Breathing": "呼吸",
  "Burst": "连发",
  "Both buttons": "双键",
  "Both directions": "双向",
  "Hall-effect · Both buttons": "霍尔效应 · 双键",
  "Freespin": "自由滚动",
  "Ratchet": "分段滚动",
  "Reverse": "反向",
  "Standard": "标准",
  "Hybrid": "混合",
  "Switches": "微动",
  "Haptics": "触感",
  "GX Safe": "GX 安全",
  "GX Speed": "GX 速度",
  "Never": "从不",
  "Left": "左",
  "Right": "右",
  "High": "高",
  "Medium": "中",
  "Low": "低",
  "Link left & right": "左右键联动",
  "Left button latency": "左键延迟",
  "Right button latency": "右键延迟",
  "Middle button latency": "中键延迟",

  // ── 补充说明 ──
  "This device has no lighting controls.": "此设备没有灯效控制项。",
  "This mouse's button settings aren't loaded yet.": "此鼠标的按键设置尚未加载。",
  "HIGH is the default frame rate; ACCEL pins the sensor to high performance at a battery cost.":
    "HIGH 为默认帧率；ACCEL 会以更耗电为代价把传感器锁定在最高性能。",
  "Write-only on this mouse: the value shown is the last one set, not read back from the dongle.":
    "此鼠标只支持写入：显示的是上次设置的值，并非从接收器读回。",
};

/** 带数字的动态文案，如 `16 ms`、`1 second`、`Device 2`。 */
function zhDynamic(s: string): string {
  return s
    .replace(/^(\d+(?:\.\d+)?) ms$/, "$1 毫秒")
    .replace(/^(\d+(?:\.\d+)?) seconds?$/, "$1 秒")
    .replace(/^(\d+(?:\.\d+)?) minutes?$/, "$1 分钟")
    .replace(/^Device (\d+)$/, "设备 $1");
}

const zh = (s: string): string => ZH[s] ?? zhDynamic(s);

/** 只改显示文案，value / id / current 一律原样保留。 */
function localizeSection(sec: Section): Section {
  return {
    title: zh(sec.title),
    controls: (sec.controls ?? []).map((c) => {
      const out: any = { ...c, label: zh(c.label) };
      if (typeof c.description === "string") out.description = zh(c.description);
      const anyC = c as any;
      if (Array.isArray(anyC.options)) {
        out.options = anyC.options.map((o: Option) => ({ ...o, label: zh(String(o.label)) }));
      }
      if (typeof anyC.zeroLabel === "string") out.zeroLabel = zh(anyC.zeroLabel);
      // info 的 current 只用于显示，可以翻译；其余是提交值，不能动。
      if (c.kind === "info" && typeof anyC.current === "string") out.current = zh(anyC.current);
      return out as Control;
    }),
  };
}


function razerAndEgg(brand: string, s: any): Section[] {
  const out: Section[] = [];
  const isRazer = brand === "Razer" && Boolean(s.razerButtonMappings);
  const isEgg = brand === "Endgame Gear" && Boolean(s.eggButtonMappings);

  if (isRazer) {
    out.push({
      title: "Razer — Buttons",
      controls: RAZER_BUTTON_CONTROLS.filter((c: string) => c !== RAZER_LOCKED_BUTTON_CONTROL).map((c: string) => ({
        id: `razer.button.${c}`,
        label: (RAZER_BUTTON_CONTROL_LABEL as any)[c],
        description: "Choose what this button sends.",
        kind: "choice" as const,
        options: opts(RAZER_BUTTON_MAPPINGS as readonly string[]),
        current: s.razerButtonMappings?.[c] ?? null,
      })),
    });
    // The Tauri tab edits these as free text; shown read-only here until the
    // app has a text field to do that with.
    out.push({
      title: "Razer — Scroll & toggles",
      controls: RAZER_TOGGLE_CONTROLS.map((c: string) => ({
        id: `razer.toggle.${c}`,
        label: (RAZER_TOGGLE_CONTROL_INFO as any)[c].label,
        description: (RAZER_TOGGLE_CONTROL_INFO as any)[c].enabledLabel,
        kind: "info" as const,
        current: String(s.razerButtonMappings?.[c] ?? "Disabled"),
      })),
    });
  }
  if (isEgg) {
    out.push({
      title: "Endgame Gear — Buttons",
      controls: (EGG_BUTTON_NAMES as readonly string[]).map((name, i) => ({
        id: `egg.button.${i}`,
        label: name,
        kind: "choice" as const,
        options: opts(EGG_BUTTON_MAPPINGS as readonly string[]),
        current: s.eggButtonMappings?.[i] ?? null,
      })),
    });
    if (Array.isArray(s.eggMulticlickFilters) && s.eggMulticlickFilters.length > 0) {
      out.push({
        title: "Endgame Gear — Debounce",
        controls: (EGG_BUTTON_NAMES as readonly string[]).slice(0, s.eggMulticlickFilters.length).map((name, i) => ({
          id: `egg.debounce.${i}`,
          label: name,
          kind: "stepped" as const,
          options: [0, 1, 2, 4, 8, 12, 16].map((v) => ({ value: v, label: v === 0 ? "Off" : `${v} ms` })),
          current: s.eggMulticlickFilters?.[i] ?? 16,
        })),
      });
    }
  }
  return out;
}

/**
 * The shared remapper: a driver that reports `buttonMappings` (display label →
 * action label) with `buttonOptions` and implements `setButtonMapping(label,
 * action)`. Buttons it reports in an encoding it can't name (`fixedButtons`, e.g.
 * macros) are shown but not editable. Razer and Endgame Gear keep their own
 * sections above; they report different fields.
 */
function genericButtons(s: any): Section[] {
  if (!s.buttonMappings || typeof s.buttonMappings !== "object" || !Array.isArray(s.buttonOptions) || s.buttonOptions.length === 0) return [];
  const fixed: string[] = Array.isArray(s.fixedButtons) ? s.fixedButtons : [];
  const controls: Control[] = Object.entries(s.buttonMappings as Record<string, string>).map(([button, action]) =>
    fixed.includes(button)
      ? { id: `gen.fixed/${button}`, label: button, description: "Set up with a macro or custom binding — not editable here.", kind: "info" as const, current: String(action) }
      : { id: `gen.button/${button}`, label: button, kind: "choice" as const, options: opts(s.buttonOptions), current: action },
  );
  return controls.length ? [{ title: "Buttons", controls }] : [];
}

function advanced(s: any, brand: string): Section[] {
  const out: Section[] = [];
  const isEgg = typeof s.slamclickFilter === "boolean" || typeof s.motionJitterFilter === "boolean";
  const eggAny = isEgg || (Array.isArray(s.eggCpiStages) && s.eggCpiStages.length > 0);

  if (eggAny) {
    const c: Control[] = [];
    if (typeof s.slamclickFilter === "boolean") c.push({ id: "egg.slamclick", label: "Slamclick filter", kind: "toggle", current: s.slamclickFilter });
    if (typeof s.motionJitterFilter === "boolean") c.push({ id: "egg.jitter", label: "Motion-jitter filter", kind: "toggle", current: s.motionJitterFilter });
    const gx = ["Off", "GX Safe", "GX Speed"];
    if (s.leftSpdtMode != null || s.rightSpdtMode != null) {
      c.push({ id: "egg.spdt.left", label: "GX switch — left button", kind: "choice", options: opts(gx), current: s.leftSpdtMode ?? "Off" });
      if (s.rightSpdtMode != null) c.push({ id: "egg.spdt.right", label: "GX switch — right button", kind: "choice", options: opts(gx), current: s.rightSpdtMode });
    }
    if (Array.isArray(s.eggCpiStages) && s.eggCpiStages.length > 0) {
      c.push({ id: "egg.cpiLevels", label: "CPI levels", kind: "choice", options: opts([1, 2, 3, 4]), current: s.eggCpiLevels ?? 1 });
    }
    if (s.eggPollingDivider != null) {
      const d = s.eggPollingDivider;
      c.push({
        id: "egg.pollingDivider",
        label: "8K polling divider",
        description: `Result: ${Number.isInteger(d) && d > 0 ? `${(8000 / d).toLocaleString(undefined, { maximumFractionDigits: 2 })} Hz` : "—"}`,
        kind: "number",
        min: 1,
        max: 255,
        current: d,
      });
    }
    out.push({ title: "Endgame Gear", controls: c });
  }

  if (s.ninjutsoSystemMode || s.ninjutsoOpticalEngine || s.ninjutsoHyperClick != null || s.ninjutsoSlamClick) {
    const c: Control[] = [];
    if (s.ninjutsoSystemMode) c.push({ id: "nin.systemMode", label: "System mode", kind: "choice", options: opts(s.ninjutsoSystemModes ?? []), current: s.ninjutsoSystemMode });
    if (s.ninjutsoOpticalEngine) c.push({ id: "nin.engine", label: "Optical Engine", kind: "choice", options: opts(["Standard", "Burst"]), current: s.ninjutsoOpticalEngine });
    if (s.ninjutsoHyperClick != null) c.push({ id: "nin.hyperclick", label: "HyperClick", kind: "toggle", current: Boolean(s.ninjutsoHyperClick) });
    if (s.ninjutsoSlamClick) c.push({ id: "nin.slamclick", label: "Slam-Click", kind: "choice", options: opts(["Low", "Medium", "High"]), current: s.ninjutsoSlamClick });
    out.push({ title: "Ninjutso", controls: c });
  }

  if (s.finalmouseDongleLedMode != null) {
    out.push({
      title: "Finalmouse",
      controls: [
        { id: "fm.dongleLed", label: "Dongle LED", kind: "choice", options: [{ value: 0, label: "Off" }, { value: 1, label: "Battery indicator" }, { value: 2, label: "Solid white" }], current: s.finalmouseDongleLedMode ?? 0 },
        { id: "fm.scrollMode", label: "Tournament scroll", kind: "choice", options: [{ value: 0, label: "Off" }, { value: 1, label: "Scroll up" }, { value: 2, label: "Scroll down" }, { value: 3, label: "Both directions" }], current: s.finalmouseTournamentScrollMode ?? 0 },
        { id: "fm.scrollTimeout", label: "Passthrough window", kind: "choice", options: [{ value: 100, label: "100 ms" }, { value: 500, label: "500 ms" }, { value: 1000, label: "1 second" }, { value: 1500, label: "1.5 seconds" }], current: s.finalmouseTournamentScrollTimeoutMs ?? 100 },
      ],
    });
  }

  if (s.sensorMode != null || s.performanceDuration != null || s.dpiLedMode != null) {
    const c: Control[] = [];
    if (s.dpiLedMode != null) {
      const modes: number[] = s.ui?.dpiLighting?.modes ?? [0, 1, 2];
      c.push({ id: "tee.dpiLed", label: "DPI indicator effect", kind: "choice", options: modes.map((m) => ({ value: m, label: m === 0 ? "Off" : m === 1 ? "Steady" : "Breathing" })), current: s.dpiLedMode });
    }
    if (s.performanceDuration != null) {
      c.push({ id: "tee.duration", label: "Performance duration", kind: "choice", options: (TEEVOLUTION_PERFORMANCE_TIME_LABELS as readonly (readonly [number, string])[]).map(([value, label]) => ({ value, label })), current: s.performanceDuration });
    }
    out.push({ title: "Teevolution", controls: c });
  }

  // The 2.4 GHz dongle / receiver: LED behaviour and, where a driver reports it,
  // the receiver's own state. Finalmouse's dongle LED has its own section above.
  const dongle: Control[] = [];
  if (typeof s.dongleLedEnabled === "boolean") {
    dongle.push({ id: "dongle.led", label: "Dongle LED", description: "The receiver's status light.", kind: "toggle", current: s.dongleLedEnabled });
  }
  if (typeof s.incottReceiverLedMode === "number") {
    dongle.push({
      id: "dongle.incott",
      label: "Receiver LED",
      description: "What the 2.4 GHz receiver's light shows.",
      kind: "choice",
      options: [{ value: 0, label: "Connection & polling rate" }, { value: 1, label: "Battery status" }, { value: 2, label: "Battery warning" }],
      current: s.incottReceiverLedMode,
    });
  }
  // Write-only: null until something has been written, but the control is still real.
  if (s.atkDongleLight !== undefined) {
    dongle.push({
      id: "dongle.atk",
      label: "Dongle light",
      description: "Write-only on this mouse: the value shown is the last one set, not read back from the dongle.",
      kind: "choice",
      options: [{ value: 0, label: "Off" }, { value: 1, label: "Polling rate" }, { value: 2, label: "Battery" }, { value: 3, label: "Low battery" }],
      current: s.atkDongleLight ?? null,
    });
  }
  if (s.atkReceiver && typeof s.atkReceiver === "object") {
    const r = s.atkReceiver;
    dongle.push({ id: "dongle.info.state", label: "Receiver", kind: "info", current: r.online ? "Connected" : "Not connected" });
    if (r.rfId) dongle.push({ id: "dongle.info.rf", label: "Receiver ID", kind: "info", current: String(r.rfId) });
    if (r.pairingSecondsRemaining) dongle.push({ id: "dongle.info.pairing", label: "Pairing", kind: "info", current: `Open for ${r.pairingSecondsRemaining} s` });
  }
  if (dongle.length) out.push({ title: "Dongle", controls: dongle });

  // A driver that reports a profile count gets a selector with exactly that many
  // (and its own profile names); Pulsar Pro, which doesn't, keeps its fixed 1-6.
  const hasProfileCount = typeof s.profileCount === "number" && s.profileCount > 1;
  if (hasProfileCount && typeof s.activeProfile === "number") {
    const names: string[] = Array.isArray(s.profileNames) ? s.profileNames : [];
    out.push({
      title: "Profiles",
      controls: [{
        id: "gen.profile",
        label: "Active profile",
        description: "Switches the mouse's stored profile.",
        kind: "choice",
        options: Array.from({ length: s.profileCount }, (_, i) => ({ value: i + 1, label: names[i] || `Profile ${i + 1}` })),
        current: s.activeProfile,
      }],
    });
  }
  if (s.wheelAcceleration != null) {
    out.push({ title: "Scroll wheel", controls: [{ id: "pulsar.wheel", label: "Wheel acceleration", kind: "toggle", current: Boolean(s.wheelAcceleration) }] });
  }
  // Pulsar's own fixed 1-6 profile selector. Only for Pulsar: other brands (Logitech
  // reports its running onboard profile as `activeProfile` too) must not get it.
  if (brand === "Pulsar" && !hasProfileCount && typeof s.activeProfile === "number" && s.activeProfile > 0) {
    out.push({ title: "Pulsar Pro", controls: [{ id: "pulsar.profile", label: "Active profile", kind: "choice", options: opts([1, 2, 3, 4, 5, 6]), current: s.activeProfile }] });
  }
  return out;
}

/** Brand-specific extras that were reported by drivers but had no control yet. */
function extras(s: any): Section[] {
  const out: Section[] = [];

  if (s.finalmouseIsSlx === true) {
    const mode = [{ value: 0, label: "Mechanical" }, { value: 1, label: "TMR analog" }];
    const rel = [{ value: 0, label: "Normal" }, { value: 1, label: "Early" }, { value: 2, label: "Late" }];
    // Edit both switches as one, or each on its own. Starts linked when they already match.
    const keys = ["ClickMode", "ClickRelease", "TmrThr", "TmrHyst"];
    if (s.slxLinked === undefined) s.slxLinked = linkLeftRight ?? keys.every((k) => s[`finalmouse${k}L`] === s[`finalmouse${k}R`]);
    const linked: boolean = s.slxLinked;
    out.push({ title: "Starlight X", controls: [{ id: "slx.link", label: "Link left & right", description: "Change both buttons together.", kind: "toggle", current: linked }] });
    for (const side of linked ? (["L"] as const) : (["L", "R"] as const)) {
      const c: Control[] = [];
      const name = linked ? "Both buttons" : side === "L" ? "Left button" : "Right button";
      const m = s[`finalmouseClickMode${side}`];
      if (m != null) c.push({ id: `slx.mode${side}`, label: "Switch", kind: "choice", options: mode, current: m });
      const r = s[`finalmouseClickRelease${side}`];
      if (r != null) c.push({ id: `slx.rel${side}`, label: "Release point", kind: "choice", options: rel, current: r });
      // Analog tuning only means something while the switch is in TMR mode.
      if (m === 1) {
        const thr = s[`finalmouseTmrThr${side}`];
        if (thr != null) c.push({ id: `slx.thr${side}`, label: "Actuation point", kind: "number", min: 0.01, max: 0.4, step: 0.01, decimals: 2, unit: "mm", current: thr / 100 });
        const rt = s[`finalmouseTmrHyst${side}`];
        if (rt != null) c.push({ id: `slx.rt${side}`, label: "Rapid trigger", kind: "number", min: 150, max: 250, step: 5, unit: "µm", current: rt });
        const msp = s[`finalmouseTmrMsp${side}`];
        if (msp != null && !linked) c.push({ id: `slx.info.msp${side}`, label: "Calibration reference", kind: "info", current: `${msp} µm` });
      }
      if (c.length) out.push({ title: `Starlight X · ${name}`, controls: c });
    }
    if (s.finalmousePawLodMm != null) out.push({ title: "Starlight X · Lift-off", controls: [{ id: "slx.lod", label: "PAW lift-off", kind: "number", min: 0.7, max: 1.7, step: 0.1, decimals: 1, unit: "mm", current: s.finalmousePawLodMm }] });
    if (typeof s.finalmouseProfileCount === "number" && s.finalmouseProfileCount > 1 && typeof s.finalmouseProfileActive === "number") {
      const names = s.finalmouseProfileNames ?? {};
      const mask = Number(s.finalmouseProfileEnabledMask ?? -1);
      const options = Array.from({ length: s.finalmouseProfileCount }, (_, i) => ({ value: i, label: names[i] || `Profile ${i + 1}` })).filter((o) => mask === -1 || (mask >> Number(o.value)) & 1 || o.value === s.finalmouseProfileActive);
      out.push({ title: "Profiles", controls: [{ id: "slx.profile", label: "Active profile", kind: "choice", options, current: s.finalmouseProfileActive }] });
    }
  }

  const atk: Control[] = [];
  if (s.atkSensorMode != null) atk.push({ id: "atk.sensorMode", label: "Sensor mode", kind: "choice", options: [{ value: 0, label: "Basic" }, { value: 1, label: "Shard" }, { value: 2, label: "Shard MAX" }], current: s.atkSensorMode });
  if (s.atkAntiMistouchMs != null) atk.push({ id: "atk.antiMistouch", label: "Anti-mistouch", description: "Ignored-click window in ms; 0 is off.", kind: "number", min: 0, max: 2550, step: 10, current: s.atkAntiMistouchMs });
  if (typeof s.atkProfileCount === "number" && s.atkProfileCount > 1 && typeof s.activeProfile === "number") {
    atk.push({ id: "atk.profile", label: "Active profile", kind: "choice", options: Array.from({ length: s.atkProfileCount }, (_, i) => ({ value: i + 1, label: `Profile ${i + 1}` })), current: s.activeProfile });
  }
  if (Array.isArray(s.atkButtonMappings)) {
    for (const b of s.atkButtonMappings) atk.push({ id: `atk.info.${b.id}`, label: `${b.name} button`, kind: "info", current: String(b.action) });
  }
  if (atk.length) out.push({ title: "ATTACK SHARK", controls: atk });

  if (s.incottFireKeyTimes != null) {
    out.push({
      title: "Fire key",
      controls: [
        { id: "incott.fireTimes", label: "Clicks per press", kind: "choice", options: opts([1, 2, 3]), current: s.incottFireKeyTimes },
        { id: "incott.fireInterval", label: "Interval", description: "Milliseconds between clicks.", kind: "number", min: 0, max: 255, step: 5, current: s.incottFireKeyIntervalMs ?? 0 },
      ],
    });
  }

  if (s.eggSupportsGlassMode && s.eggGlassMode != null) {
    out.push({ title: "Endgame Gear", controls: [{ id: "egg.glass", label: "Glass mode", description: "Tracking tuned for glass surfaces.", kind: "toggle", current: Boolean(s.eggGlassMode) }] });
  }

  if (s.asusOmni && typeof s.asusOmni === "object") {
    const o = s.asusOmni;
    const c: Control[] = [];
    if (o.firmware) c.push({ id: "omni.info.fw", label: "Receiver firmware", kind: "info", current: String(o.firmware) });
    for (const [i, d] of (o.devices ?? []).entries()) c.push({ id: `omni.info.dev${i}`, label: d.name || `Device ${i + 1}`, kind: "info", current: String(d.kind ?? "") });
    c.push({ id: "omni.pair", label: "Pairing", description: "Open the receiver for a new device.", kind: "choice", options: [{ value: 1, label: "Start pairing" }, { value: 0, label: "Stop" }], current: null });
    out.push({ title: "Omni receiver", controls: c });
  }

  const lf = s.lunafury;
  if (lf && typeof lf === "object") {
    const c: Control[] = [];
    if (lf.lightningMode != null) c.push({ id: "lf.lightning", label: "Lightning trigger", kind: "choice", options: opts([0, 1, 2]), current: lf.lightningMode });
    for (const b of ["left", "right", "middle"]) {
      const v = lf[`${b}DebounceMs`];
      if (v != null) c.push({ id: `lf.debounce.${b}`, label: `${b[0].toUpperCase()}${b.slice(1)} button latency`, description: "Milliseconds.", kind: "number", min: 0, max: 20, step: 1, current: v });
    }
    if (lf.wheelGuard) {
      c.push({ id: "lf.guard", label: "Wheel guard", kind: "toggle", current: Boolean(lf.wheelGuard.enabled) });
      c.push({ id: "lf.guardMs", label: "Wheel guard window", description: "Milliseconds.", kind: "number", min: 0, max: 500, step: 10, current: lf.wheelGuard.windowMs ?? 0 });
    }
    if (lf.bottomButtonMode != null) c.push({ id: "lf.bottom", label: "Bottom button", kind: "choice", options: opts([1, 2]), current: lf.bottomButtonMode });
    if (lf.receiverLightMode != null) c.push({ id: "lf.receiverLight", label: "Receiver light", kind: "choice", options: opts([0, 2, 6, 11]), current: lf.receiverLightMode });
    if (lf.buttonPollingRateHz != null) c.push({ id: "lf.buttonRate", label: "Button polling rate", kind: "choice", options: opts([...(lf.supportedButtonPollingRates ?? [lf.buttonPollingRateHz])]), current: lf.buttonPollingRateHz });
    if (lf.separateDpiAxes != null) c.push({ id: "lf.separateAxes", label: "Separate DPI axes", kind: "toggle", current: Boolean(lf.separateDpiAxes) });
    if (c.length) out.push({ title: "LunaFury", controls: c });
  }
  return out;
}

/** Logitech PRO X 3 Superstrike: hall-effect (HITS) tuning per primary button. */
function hits(s: any, brand: string): Section[] {
  const t = s.analogButtonTuning;
  if (brand !== "Logitech" || !t || !Array.isArray(t.buttons) || t.buttons.length === 0) return [];
  const same = (k: string) => t.buttons.every((b: any) => b[k] === t.buttons[0][k]);
  if (s.hitsLinked === undefined) s.hitsLinked = t.buttons.length > 1 && (linkLeftRight ?? ["actuation", "rapidTrigger", "haptics", "rapidTriggerEnabled"].every(same));
  const linked: boolean = s.hitsLinked && t.buttons.length > 1;
  const out: Section[] = [];
  if (t.buttons.length > 1) {
    out.push({ title: "Hall-effect buttons", controls: [{ id: "hits.link", label: "Link left & right", description: "Change both buttons together.", kind: "toggle", current: linked }] });
  }
  const names = ["Left button", "Right button"];
  for (const i of linked ? [0] : t.buttons.keys()) {
    const b = t.buttons[i];
    const c: Control[] = [
      { id: `hits.act.${i}`, label: "Actuation point", description: "Higher means a deeper press.", kind: "number", min: 1, max: t.maxActuation, step: 1, current: b.actuation },
      // One slider: 0 is off, 1 and up is the sensitivity.
      { id: `hits.rt.${i}`, label: "Rapid trigger", description: "How far the button must lift to reset. Off at 0.", kind: "number", min: 0, max: t.maxRapidTrigger, step: 1, zeroLabel: "Off", current: b.rapidTriggerEnabled ? b.rapidTrigger : 0 },
    ];
    if (t.maxHaptics > 0) c.push({ id: `hits.hap.${i}`, label: "Click haptics", description: "Strength of this button's click feedback.", kind: "number", min: 0, max: t.maxHaptics, step: 1, current: b.haptics });
    out.push({ title: linked ? "Hall-effect · Both buttons" : `Hall-effect · ${names[i] ?? `Button ${i + 1}`}`, controls: c });
  }
  return out;
}

/**
 * Settings a driver reports under a shared status field and sets with a method of
 * the same name (`wheelMode` / `setWheelMode`, and so on). Each is offered when the
 * device reports the field, and written through whichever driver answered it.
 * Grouped by what a person would call them; all apply immediately.
 */
type GenericSpec = {
  field: string;
  method: string;
  section: string;
  label: string;
  description?: string;
  /** toggle / choice (with `options`) / number (with `min`/`max`/`step`). */
  kind: "toggle" | "choice" | "number";
  options?: readonly (string | number)[];
  min?: number;
  max?: number;
  step?: number;
  /** Extra condition beyond the field being reported. */
  when?: (status: any) => boolean;
};

const GENERIC: GenericSpec[] = [
  // Scroll wheel (Logitech, Ajazz, Motospeed, K'snake)
  { field: "wheelMode", method: "setWheelMode", section: "Scroll wheel", label: "Wheel mode", description: "Free-spinning or ratcheted, as the button behind the wheel toggles.", kind: "choice", options: ["Freespin", "Ratchet"] },
  { field: "smartShiftThreshold", method: "setSmartShiftThreshold", section: "Scroll wheel", label: "SmartShift sensitivity", description: "How gentle a flick releases the ratchet. 255 turns SmartShift off.", kind: "number", min: 1, max: 255, step: 5 },
  { field: "hiResScroll", method: "setHiResScroll", section: "Scroll wheel", label: "High-resolution scrolling", kind: "toggle" },
  { field: "invertScroll", method: "setInvertScroll", section: "Scroll wheel", label: "Invert scroll direction", kind: "toggle", when: (s) => s.supportsInvertScroll !== false },
  { field: "thumbWheelInverted", method: "setThumbWheelInverted", section: "Scroll wheel", label: "Invert thumb wheel", kind: "toggle", when: (s) => s.supportsThumbWheelInvert !== false },
  { field: "scrollDirection", method: "setScrollDirection", section: "Scroll wheel", label: "Scroll direction", kind: "choice", options: ["Forward", "Reverse"] },
  // Switches and haptics
  { field: "lightforceSwitchMode", method: "setLightforceSwitchMode", section: "Switches", label: "Lightforce switches", description: "Hybrid uses an optical and a mechanical stage; Optical is purely optical.", kind: "choice", options: ["Hybrid", "Optical"] },
  { field: "hapticEnabled", method: "setHapticEnabled", section: "Haptics", label: "Haptic feedback", kind: "toggle" },
  { field: "hapticIntensity", method: "setHapticIntensity", section: "Haptics", label: "Haptic strength", kind: "number", min: 0, max: 100, step: 5 },
  { field: "hapticBatterySaving", method: "setHapticBatterySaving", section: "Haptics", label: "Haptics battery saving", kind: "toggle" },
  // Sensor modes
  { field: "hyperMode", method: "setHyperMode", section: "Sensor", label: "Hyper mode", description: "Runs the sensor at its highest performance setting.", kind: "toggle" },
  { field: "turboMode", method: "setTurboMode", section: "Sensor", label: "Turbo mode", description: "Pins the sensor to its highest frame rate.", kind: "toggle" },
  { field: "sensorScanningMode", method: "setSensorScanningMode", section: "Sensor", label: "Sensor scanning", description: "HIGH is the default frame rate; ACCEL pins the sensor to high performance at a battery cost.", kind: "choice", options: ["HIGH", "ACCEL"] },
  { field: "dynamicSensitivityEnabled", method: "setDynamicSensitivityEnabled", section: "Sensor", label: "DPI acceleration", description: "Needs mouse firmware 57 or later.", kind: "toggle" },
  { field: "dynamicSensitivityMode", method: "setDynamicSensitivityMode", section: "Sensor", label: "Acceleration curve", kind: "choice", options: ["classic", "natural", "jump", "custom"] },
  { field: "buttonCombination", method: "setButtonCombination", section: "Sensor", label: "Button combinations", description: "Lets button chords change mouse settings without the driver.", kind: "toggle" },
  { field: "dpiLedSleepTimeout", method: "setDpiLedSleepTimeout", section: "Sensor", label: "DPI light sleep", description: "Seconds before the DPI indicator turns off, separate from mouse sleep.", kind: "number", min: 0, max: 600, step: 5 },
  // Wireless
  { field: "longRangeMode", method: "setLongRangeMode", section: "Wireless", label: "Long-range mode", description: "Trades battery for link range on receivers that offer it.", kind: "toggle" },
];

const titleCase = (value: string) => value.charAt(0).toUpperCase() + value.slice(1);

/**
 * Hall-effect primary buttons (G-Wolves, Lamzu): where the click registers,
 * rapid trigger, and which switch each button uses. `mag.<field>.<button>`,
 * with button 0 the left and 1 the right.
 */
function magneticControls(s: any): Section[] {
  const m = s.magneticButtons;
  if (!m || !Array.isArray(m.buttons) || m.buttons.length === 0) return [];
  const rtUnit = m.rapidTriggerUnit === "ms" ? "ms" : undefined;
  return m.buttons.map((b: any, i: number): Section => {
    const controls: Control[] = [];
    if (m.canChooseSwitchType && b.switchType) {
      controls.push({
        id: `mag.switch.${i}`,
        label: "Switch",
        description: "Magnetic needs a calibrated mouse.",
        kind: "choice",
        options: [{ value: "magnetic", label: "Magnetic" }, { value: "optical", label: "Optical" }],
        current: b.switchType,
      });
    }
    if (b.triggerPoint != null && m.triggerPointRange) {
      controls.push({
        id: `mag.trigger.${i}`,
        label: "Trigger point",
        description: "How far the button travels before it clicks. Lower is lighter.",
        kind: "number",
        min: m.triggerPointRange.min,
        max: m.triggerPointRange.max,
        step: 1,
        current: b.triggerPoint,
      });
    }
    if (m.releasePointRange) {
      controls.push({
        id: `mag.release.${i}`,
        label: "Release point",
        description: "How far the button travels back before it releases.",
        kind: "number",
        min: m.releasePointRange.min,
        max: m.releasePointRange.max,
        step: 1,
        current: b.releasePoint ?? b.triggerPoint ?? m.releasePointRange.min,
      });
    }
    if (m.rapidTriggerRange) {
      const on = b.rapidTriggerEnabled ?? (b.rapidTrigger != null && b.rapidTrigger > 0);
      controls.push({ id: `mag.rtOn.${i}`, label: "Rapid trigger", description: "Re-clicks the moment the button moves back up.", kind: "toggle", current: Boolean(on) });
      if (on) {
        controls.push({
          id: `mag.rt.${i}`,
          label: "Rapid trigger sensitivity",
          kind: "number",
          min: m.rapidTriggerRange.min,
          max: m.rapidTriggerRange.max,
          step: 1,
          unit: rtUnit,
          current: b.rapidTrigger ?? m.rapidTriggerRange.min,
        });
      }
    } else if (m.rapidTriggerSwitch === false || b.rapidTrigger != null) {
      controls.push({ id: `mag.info.rt${i}`, label: "Rapid trigger", description: "Can only be changed with the mouse on its cable.", kind: "info", current: b.rapidTriggerEnabled ? "On" : "Off" });
    }
    return { title: m.buttons.length === 2 ? `${i === 0 ? "Left" : "Right"} button` : `Button ${i + 1}`, controls };
  }).filter((section: Section) => section.controls.length > 0);
}

/** Writes one magnetic control; the driver confirms by reading the value back. */
async function writeMagnetic(info: HidInterfaceInfo, status: any, id: string, value: any) {
  const [, field, rawButton] = id.split(".");
  const button = Number(rawButton) as 0 | 1;
  const m = status.magneticButtons;
  const b = m?.buttons?.[button];
  if (!m || !b) throw new Error("This mouse's button settings aren't loaded yet.");
  const min = m.rapidTriggerRange?.min ?? 1;
  switch (field) {
    case "trigger":
      await callClientMethod(info, "setMagneticTriggerPoint", button, Math.round(Number(value)));
      b.triggerPoint = Math.round(Number(value));
      return;
    case "release":
      await callClientMethod(info, "setMagneticReleasePoint", button, Math.round(Number(value)));
      b.releasePoint = Math.round(Number(value));
      return;
    case "rtOn": {
      const level = b.rapidTrigger != null && b.rapidTrigger >= min ? b.rapidTrigger : min;
      await callClientMethod(info, "setMagneticRapidTrigger", button, Boolean(value), level);
      b.rapidTriggerEnabled = Boolean(value);
      b.rapidTrigger = level;
      return;
    }
    case "rt":
      await callClientMethod(info, "setMagneticRapidTrigger", button, true, Math.round(Number(value)));
      b.rapidTriggerEnabled = true;
      b.rapidTrigger = Math.round(Number(value));
      return;
    case "switch": {
      const types = m.buttons.map((x: any) => x.switchType);
      types[button] = String(value);
      await callClientMethod(info, "setMagneticSwitchTypes", types);
      b.switchType = String(value);
      return;
    }
    default:
      throw new Error(`Unknown magnetic control ${id}`);
  }
}

function genericFeatures(status: any): Section[] {
  const sections = new Map<string, Control[]>();
  for (const spec of GENERIC) {
    const value = status[spec.field];
    if (value === undefined || value === null) continue;
    if (spec.when && !spec.when(status)) continue;
    const id = `gen.set/${spec.field}/${spec.method}`;
    let control: Control | null = null;
    if (spec.kind === "toggle" && typeof value === "boolean") {
      control = { id, label: spec.label, description: spec.description, kind: "toggle", current: value };
    } else if (spec.kind === "choice" && spec.options) {
      control = {
        id, label: spec.label, description: spec.description, kind: "choice",
        options: spec.options.map((o) => ({ value: o, label: typeof o === "string" ? titleCase(o) : String(o) })),
        current: value,
      };
    } else if (spec.kind === "number" && typeof value === "number") {
      control = { id, label: spec.label, description: spec.description, kind: "number", min: spec.min ?? 0, max: spec.max ?? 100, step: spec.step, current: value };
    }
    if (control) sections.set(spec.section, [...(sections.get(spec.section) ?? []), control]);
  }
  return [...sections.entries()].map(([title, controls]) => ({ title, controls }));
}

/** Read-only facts a device reports about itself: identity, link and Easy-Switch state. */
function deviceInfo(status: any): Section[] {
  const c: Control[] = [];
  const info = (id: string, label: string, current: string | number | null | undefined, unit = "") => {
    if (current !== undefined && current !== null && current !== "") c.push({ id: `info.${id}`, label, kind: "info", current: `${current}${unit}` });
  };
  info("unit", "Unit ID", status.unitId);
  info("model", "Model ID", status.modelId);
  if (status.transportIds && typeof status.transportIds === "object") {
    info("transports", "Transports", Object.entries(status.transportIds as Record<string, string>).map(([k, v]) => `${k} ${v}`).join(" · "));
  }
  info("connection", "Connection", status.connectionDetail);
  info("signal", "Signal strength", status.signalStrength, "%");
  info("voltage", "Battery voltage", status.batteryVoltageMv, " mV");
  info("name", "Device name", status.friendlyName);
  if (typeof status.hostCount === "number" && status.hostCount > 0) {
    const paired = Array.isArray(status.hostSlotsPaired) ? status.hostSlotsPaired.filter(Boolean).length : null;
    const slot = typeof status.currentHost === "number" ? `slot ${status.currentHost + 1} of ${status.hostCount}` : `${status.hostCount} slots`;
    info("hosts", "Easy-Switch", paired === null ? slot : `${slot} · ${paired} paired`);
  }
  info("nape", "Layer", typeof status.napeLayer === "number" && status.napeLayerCount ? `${status.napeLayer + 1} of ${status.napeLayerCount}` : null);
  return c.length ? [{ title: "Device info", controls: c }] : [];
}

/** "Never", "45 s", "5 min", "2 hr" for a sleep timeout in seconds. */
function sleepLabel(seconds: number): string {
  if (seconds === 0) return "Never";
  if (seconds >= 3600) return `${Math.round(seconds / 3600)} hr`;
  if (seconds >= 60) return `${Math.round(seconds / 60)} min`;
  return `${seconds} s`;
}

/** A driver's option list with the device's own current value kept in it, so a
 *  mouse sitting on something off-list still shows the truth. */
function withCurrent(steps: readonly number[], current: number | null | undefined): number[] {
  return current != null && !steps.includes(current) ? [...steps, current].sort((a, b) => a - b) : [...steps];
}

/**
 * The Performance tab's controls beyond DPI / polling / lift-off / surface (those
 * have dedicated UI): sleep, the sensor-processing switches, on-device DPI stages,
 * debounce, low-power threshold, USB speed and primary button. Every control is
 * gated exactly as src/components/DevicePerformanceTab.tsx gates it — a status
 * field the driver reports, minus whatever the driver's own `ui` hints hide — and
 * all of them apply immediately, as they do there.
 */
/** Brands whose drivers implement `setAngleTuning` (the others only report the angle). */
const ANGLE_TUNING_WRITERS = new Set(["MCHOSE", "Pulsar", "WLMouse"]);

function canWriteAngleTuning(brand: string, status: any): boolean {
  return ANGLE_TUNING_WRITERS.has(brand) || status.mockAngleTuningWritable === true;
}

function performanceExtras(brand: string, status: any, caps: DeviceCapabilities): Section[] {
  const ui = status.ui ?? {};
  const c: Control[] = [];

  const hasSleep = typeof status.sleepTimeout === "number" && !ui.hideSleepCard && caps.sleepTimeouts !== null;
  if (hasSleep) {
    c.push({
      id: "perf.sleep",
      label: "Sleep timeout",
      description: "How long the mouse stays awake when idle before sleeping.",
      kind: "choice",
      options: withCurrent(caps.sleepTimeouts ?? [], status.sleepTimeout).map((v) => ({ value: v, label: sleepLabel(v) })),
      current: status.sleepTimeout,
    });
  }

  const toggles: { key: string; label: string; description: string; hidden: boolean }[] = [
    { key: "motionSync", label: "Motion sync", description: "Synchronizes X/Y sensor sampling for smoother diagonals.", hidden: !!ui.hideMotionSync },
    { key: "angleSnapping", label: "Angle snapping", description: "Rounds movement to straight lines and diagonals for a steadier cursor.", hidden: !!ui.hideAngleSnapping },
    { key: "rippleControl", label: "Ripple control", description: "Filters sensor ripple at high polling rates for more stable tracking.", hidden: !!ui.hideRippleControl },
    { key: "performanceMode", label: "Performance / hyper mode", description: "Runs the sensor at its full performance setting.", hidden: false },
  ];
  if (!ui.hideProcessingCard) {
    for (const t of toggles) {
      if (!t.hidden && typeof status[t.key] === "boolean") {
        c.push({ id: `perf.${t.key}`, label: t.label, description: t.description, kind: "toggle", current: status[t.key] });
      }
    }
  }

  // Rotates the sensor's idea of "straight", in whole degrees; the driver checks
  // its own limits when written. Only some drivers that report it can write it.
  if (typeof status.angleTuning === "number" && canWriteAngleTuning(brand, status)) {
    c.push({
      id: "perf.angleTuning",
      label: "Angle tuning",
      description: "Rotates the sensor to correct for how you hold the mouse. 0° is off.",
      kind: "number",
      min: -30,
      max: 30,
      step: 1,
      unit: "°",
      current: status.angleTuning,
    });
  }

  if (Array.isArray(status.dpiStages) && status.dpiStages.length > 1) {
    c.push({
      id: "perf.dpiStage",
      label: "DPI stages",
      description: "On-device sensitivity stages; pick the active one.",
      kind: "choice",
      options: status.dpiStages.map((_: unknown, i: number) => ({ value: i, label: `Stage ${i + 1}` })),
      current: status.activeDpiStage ?? null,
    });
  }

  if (typeof status.debounceMs === "number") {
    const values = caps.debounceOptions ?? (caps.debounceMaxMs !== null ? Array.from({ length: caps.debounceMaxMs + 1 }, (_, ms) => ms) : null);
    if (values && values.length) {
      c.push({
        id: "perf.debounce",
        label: "Debounce",
        description: `Filters out physical switch bounce (0–${values[values.length - 1]} ms). Higher is steadier, slightly more click lag.`,
        kind: "stepped",
        options: values.map((ms) => ({ value: ms, label: `${ms} ms` })),
        current: status.debounceMs,
      });
    }
  }

  if (typeof status.lowBatteryWarning === "number") {
    c.push({
      id: "perf.lowPower",
      label: "Low power mode",
      description: "Slows the mouse down to save battery below this level.",
      kind: "choice",
      options: withCurrent(caps.lowPowerThresholds ?? [5, 10, 15, 20, 25, 30], status.lowBatteryWarning).map((p) => ({ value: p, label: `${p}%` })),
      current: status.lowBatteryWarning,
    });
  }

  if (Array.isArray(status.powerModes) && status.powerModes.length > 0 && typeof status.powerMode === "string") {
    c.push({
      id: "gen.powerMode",
      label: "Power mode",
      description: "Trades battery life against performance.",
      kind: "choice",
      options: opts(status.powerModes),
      current: status.powerMode,
    });
  }

  if (status.usbSpeed != null) {
    c.push({
      id: "perf.usbSpeed",
      label: "USB speed",
      description: "Full pins polling to 1000 Hz; High enables faster rates.",
      kind: "choice",
      options: opts(["Full", "High"]),
      current: status.usbSpeed,
    });
  }

  if (status.primaryButton != null) {
    c.push({
      id: "perf.primaryButton",
      label: "Primary button",
      description: "Which button acts as the primary click.",
      kind: "choice",
      options: opts(["Left", "Right"]),
      current: status.primaryButton,
    });
  }

  const out: Section[] = [];
  // Starlight X has no selectable polling rate; say why instead of leaving a gap.
  if (status.finalmouseIsSlx === true) {
    out.push({
      title: "Polling rate",
      controls: [{
        id: "slx.info.perfectPolling",
        label: "Perfect Polling",
        description: "This mouse runs a fixed-rate pipeline that keeps reports perfectly timed, so the polling rate is managed for you and can't be changed.",
        kind: "info",
        current: "Always on",
      }],
    });
  }
  if (c.length) out.push({ title: "More settings", controls: c });
  return out;
}

export function describeControls(brand: string, status: any, caps: DeviceCapabilities) {
  return {
    buttons: [...razerAndEgg(brand, status), ...genericButtons(status)].map(localizeSection),
    advanced: [...advanced(status, brand), ...hits(status, brand), ...magneticControls(status), ...extras(status), ...genericFeatures(status), ...deviceInfo(status)].map(localizeSection),
    performance: performanceExtras(brand, status, caps).map(localizeSection),
    lighting: status.lighting ?? null,
  };
}

/** Writes HITS tuning live for the affected buttons, then saves both into the running profile. */
async function writeHits(info: HidInterfaceInfo, status: any, id: string, value: any) {
  const t = status.analogButtonTuning;
  const [, field, idx] = id.split(".");
  if (field === "link") {
    status.hitsLinked = Boolean(value);
    if (!status.hitsLinked) return;
    // Linking makes the right button match the left one.
    for (let i = 1; i < t.buttons.length; i += 1) t.buttons[i] = { ...t.buttons[0] };
    return applyHits(info, t, t.buttons.keys());
  }
  const i = Number(idx);
  const touched = status.hitsLinked ? [...t.buttons.keys()] : [i];
  for (const n of touched) {
    const b = t.buttons[n];
    if (field === "act") b.actuation = Number(value);
    else if (field === "hap") b.haptics = Number(value);
    else if (field === "rt") {
      // 0 switches rapid trigger off but keeps the stored sensitivity.
      b.rapidTriggerEnabled = Number(value) > 0;
      if (Number(value) > 0) b.rapidTrigger = Number(value);
    } else throw new Error(`Unknown control ${id}`);
  }
  return applyHits(info, t, touched);
}

async function applyHits(info: HidInterfaceInfo, t: any, buttons: Iterable<number>) {
  if (info.key.startsWith("mock:")) return;
  const touched = [...buttons];
  await withLogitechClient(info, "set hall-effect buttons", async (client: any) => {
    for (const n of touched) await client.setAnalogButtonTuning(n, t.buttons[n]);
    // The live write is lost on a power cycle; keep the stored profile in step.
    await client.persistAnalogButtonTuning(t.buttons.map((b: any, button: number) => ({ button, ...b })));
  });
}

/** Writes one control by id (the dispatch for everything `describeControls` emits). */
export async function writeControl(info: HidInterfaceInfo, status: any, id: string, value: any): Promise<unknown> {
  // The shared remapper: the button's display label is the part after the slash.
  if (id.startsWith("gen.button/")) return w.setButtonMapping(info, id.slice("gen.button/".length), value);
  if (id.startsWith("hits.")) return writeHits(info, status, id, value);
  if (id.startsWith("mag.") && !id.startsWith("mag.info.")) return writeMagnetic(info, status, id, value);
  // Table-driven generic setting: `gen.set/<field>/<method>`.
  if (id.startsWith("gen.set/")) return callClientMethod(info, id.split("/")[2], value);
  const [family, kind, rest] = id.split(".");
  // Logitech button remap: `logi.button.<layer>.<n>` edits the running onboard
  // profile. The library checks the profile's CRC and format first, skips the
  // flash write when nothing changed, reads the sector back, and restores the
  // old one if that fails.
  if (family === "logi" && kind === "button") {
    const [, , layer, n] = id.split(".");
    if (info.key.startsWith("mock:")) return;
    return withLogitechClient(info, "set button", (client: any) =>
      client.writeActiveProfile({
        buttonAssignments: [{ layer: layer === "g-shift" ? "g-shift" : "primary", button: Number(n), binding: { kind: "action", action: value } }],
      }),
    );
  }
  switch (id) {
    case "slx.link": {
      status.slxLinked = Boolean(value);
      if (status.slxLinked) {
        // Linking makes the right button match the left one.
        const l = (k: string) => status[`finalmouse${k}L`];
        await callClientMethod(info, "setClickMode", l("ClickMode"), l("ClickMode"), l("ClickRelease"), l("ClickRelease"));
        if (l("TmrThr") != null) await callClientMethod(info, "setTmrActuation", l("TmrThr") / 100, l("TmrThr") / 100, l("TmrHyst"), l("TmrHyst"));
        for (const k of ["ClickMode", "ClickRelease", "TmrThr", "TmrHyst", "TmrMsp"]) status[`finalmouse${k}R`] = status[`finalmouse${k}L`];
      }
      return;
    }
    case "slx.modeL": case "slx.modeR": case "slx.relL": case "slx.relR": {
      const m = { L: status.finalmouseClickModeL ?? 0, R: status.finalmouseClickModeR ?? 0 };
      const r = { L: status.finalmouseClickReleaseL ?? 0, R: status.finalmouseClickReleaseR ?? 0 };
      const side = id.endsWith("L") ? "L" : "R";
      const target = id.startsWith("slx.mode") ? m : r;
      target[side] = Number(value);
      if (status.slxLinked) target.L = target.R = Number(value);
      await callClientMethod(info, "setClickMode", m.L, m.R, r.L, r.R);
      status.finalmouseClickModeL = m.L; status.finalmouseClickModeR = m.R;
      status.finalmouseClickReleaseL = r.L; status.finalmouseClickReleaseR = r.R;
      return;
    }
    case "slx.thrL": case "slx.thrR": case "slx.rtL": case "slx.rtR": {
      const t = { L: (status.finalmouseTmrThrL ?? 20) / 100, R: (status.finalmouseTmrThrR ?? 20) / 100 };
      const h = { L: status.finalmouseTmrHystL ?? 150, R: status.finalmouseTmrHystR ?? 150 };
      const side = id.endsWith("L") ? "L" : "R";
      const target = id.startsWith("slx.thr") ? t : h;
      target[side] = id.startsWith("slx.thr") ? Math.round(Number(value) * 100) / 100 : Number(value);
      if (status.slxLinked) target.L = target.R = target[side];
      await callClientMethod(info, "setTmrActuation", t.L, t.R, h.L, h.R);
      status.finalmouseTmrThrL = Math.round(t.L * 100); status.finalmouseTmrThrR = Math.round(t.R * 100);
      status.finalmouseTmrHystL = h.L; status.finalmouseTmrHystR = h.R;
      return;
    }
    case "slx.lod": await callClientMethod(info, "setPawLodMm", Math.round(Number(value) * 10) / 10); status.finalmousePawLodMm = Number(value); return;
    case "slx.profile": await callClientMethod(info, "setActiveProfile", Number(value)); status.finalmouseProfileActive = Number(value); return;
    case "atk.sensorMode": await callClientMethod(info, "setAtkSensorMode", Number(value)); status.atkSensorMode = Number(value); return;
    case "atk.antiMistouch": await callClientMethod(info, "setAntiMistouchMs", Number(value)); status.atkAntiMistouchMs = Number(value); return;
    case "atk.profile": await callClientMethod(info, "setR1ActiveProfile", Number(value)); status.activeProfile = Number(value); return;
    case "incott.fireTimes": case "incott.fireInterval": {
      const times = id === "incott.fireTimes" ? Number(value) : status.incottFireKeyTimes ?? 1;
      const ms = id === "incott.fireInterval" ? Number(value) : status.incottFireKeyIntervalMs ?? 0;
      await callClientMethod(info, "setFireKey", times, ms);
      status.incottFireKeyTimes = times; status.incottFireKeyIntervalMs = ms;
      return;
    }
    case "egg.glass": await callClientMethod(info, "setGlassMode", Boolean(value)); status.eggGlassMode = Boolean(value); return;
    case "omni.pair": return callClientMethod(info, "setOmniPairingMode", Number(value) === 1);
    case "lf.lightning": return callClientMethod(info, "setLunaFuryLightningMode", Number(value));
    case "lf.bottom": return callClientMethod(info, "setLunaFuryBottomButtonMode", Number(value));
    case "lf.receiverLight": return callClientMethod(info, "setLunaFuryReceiverLightMode", Number(value));
    case "lf.buttonRate": return callClientMethod(info, "setLunaFuryButtonPollingRate", Number(value));
    case "lf.separateAxes": return callClientMethod(info, "setLunaFurySeparateDpiAxes", Boolean(value));
    case "lf.guard": case "lf.guardMs": {
      const g = status.lunafury?.wheelGuard ?? { enabled: false, windowMs: 0 };
      const next = id === "lf.guard" ? { ...g, enabled: Boolean(value) } : { ...g, windowMs: Number(value) };
      await callClientMethod(info, "setLunaFuryWheelGuard", next);
      status.lunafury.wheelGuard = next;
      return;
    }
    case "dongle.led": return callClientMethod(info, "setDongleLed", Boolean(value));
    case "dongle.incott": return callClientMethod(info, "setReceiverLed", Number(value));
    case "dongle.atk": return callClientMethod(info, "setDongleLight", Number(value));
    case "gen.profile": return w.setProfile(info, Number(value));
    case "gen.powerMode": return callClientMethod(info, "setPowerMode", value);
    case "perf.sleep": return w.setSleepTimeout(info, Number(value));
    case "perf.motionSync": return w.setMotionSync(info, Boolean(value));
    case "perf.angleSnapping": return w.setAngleSnapping(info, Boolean(value));
    case "perf.angleTuning": await callClientMethod(info, "setAngleTuning", Math.round(Number(value))); status.angleTuning = Math.round(Number(value)); return;
    case "perf.rippleControl": return w.setRippleControl(info, Boolean(value));
    case "perf.performanceMode": return w.setPerformanceMode(info, Boolean(value));
    case "perf.dpiStage": return w.setActiveDpiStage(info, Number(value));
    case "perf.debounce": return w.setDebounceTime(info, Number(value));
    case "perf.lowPower": return w.setLowPowerThreshold(info, Number(value));
    case "perf.usbSpeed": return w.setUsbSpeed(info, value);
    case "perf.primaryButton": return w.setPrimaryButton(info, value);
    case "egg.slamclick": return w.setSlamclickFilter(info, value);
    case "egg.jitter": return w.setMotionJitterFilter(info, value);
    case "egg.spdt.left": return w.setSpdtMode(info, "left", value);
    case "egg.spdt.right": return w.setSpdtMode(info, "right", value);
    case "egg.cpiLevels": return w.setCpiLevels(info, Number(value));
    case "egg.pollingDivider": return w.setCustomPollingDivider(info, Number(value));
    case "nin.systemMode": return w.setNinjutsoSystemMode(info, value);
    case "nin.engine": return w.setNinjutsoOpticalEngine(info, value);
    case "nin.hyperclick": return w.setNinjutsoHyperClick(info, Boolean(value));
    case "nin.slamclick": return w.setNinjutsoSlamClick(info, value);
    case "fm.dongleLed": return w.setDongleLedMode(info, Number(value));
    case "fm.scrollMode": return w.setTournamentScrollMode(info, Number(value));
    case "fm.scrollTimeout": return w.setTournamentScrollTimeout(info, Number(value));
    case "tee.dpiLed": return w.setDpiLighting(info, Number(value), status.dpiLedBrightness ?? 0, status.dpiLedSpeed ?? 0);
    case "tee.duration": return w.setPerformanceDuration(info, Number(value));
    case "pulsar.wheel": return w.setWheelAcceleration(info, Boolean(value));
    case "pulsar.profile": return w.setProfile(info, Number(value));
  }
  if (id.startsWith("lf.debounce.")) return callClientMethod(info, "setLunaFuryButtonDebounce", id.split(".")[2], Number(value));
  if (family === "razer" && kind === "button") return w.setButtonMapping(info, rest as any, value);
  if (family === "egg" && kind === "button") return w.setButtonMapping(info, Number(rest), value);
  if (family === "egg" && kind === "debounce") return w.setMulticlickFilter(info, Number(rest), Number(value));
  throw new Error(`Unknown control ${id}`);
}

/** The lighting write DeviceLightingTab.applyAll does, from the edited fields. */
export async function writeLighting(info: HidInterfaceInfo, status: any, edit: { mode: string; color?: string | null; color2?: string | null; speed?: number | null }) {
  const l = status.lighting;
  if (!l) throw new Error("This device has no lighting controls.");
  const showColor = l.colorModes.includes(edit.mode);
  const showDual = l.dualColorModes.includes(edit.mode);
  const showSpeed = l.reactiveModes.includes(edit.mode);
  return w.setLighting(info, {
    zone: l.zone,
    modes: l.modes,
    mode: edit.mode,
    color: showColor ? edit.color ?? l.color ?? "#ff0000" : null,
    color2: showDual ? edit.color2 ?? l.color2 ?? "#0000ff" : null,
    colorModes: l.colorModes,
    dualColorModes: l.dualColorModes,
    reactiveModes: l.reactiveModes,
    speeds: l.speeds,
    speed: showSpeed ? edit.speed ?? l.speed ?? 1 : null,
    writeOnly: l.writeOnly,
  } as any);
}
