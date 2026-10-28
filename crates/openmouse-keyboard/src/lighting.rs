//! 灯效数据：主灯效 16 字节、侧灯/灯箱 24 字节、自定义灯效 512 字节。
//!
//! 字段顺序取自官方驱动 `getLEDEffect` / `getSideLight` / `getCustomLEDData`，
//! 已在真机读通（见 `examples/read.rs`）。
//!
//! **写回是无损的**：结构体内部保留读回来的整段原始字节，`encode()` 以它为底
//! 只覆盖已建模的字段。官方协议里有些字节（灯效 `[13]`、侧灯 `[4..8]` 和
//! `[11..24]`）没有名字，但设备认，清掉可能让灯效判校验失败。

const LED_EFFECT_LEN: usize = 16;
const SIDE_LIGHT_LEN: usize = 24;

/// 一个灯效模式的元数据。
///
/// 取自官方驱动前端的 `lightingModeList`（NUT75 用 `default` 协议那份），
/// 再按机型的 `customEffect: [23, 24, 25]` 追加三个新特效——所以 NUT75 的
/// 模式表是「12 个基础模式 + 3 个追加模式」，共 15 个。
///
/// `show_speed` / `show_color` / `show_direction` 也是官方那套开关：静态恒亮
/// 没有速度，流光不吃主色，彩虹风车可以调方向。界面照着这些开关决定显示
/// 哪些控件，不要统一全给——给了也没用，写进去设备也不理。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectMode {
    pub id: u8,
    pub name: &'static str,
    pub show_speed: bool,
    pub show_color: bool,
    pub show_direction: bool,
}

/// NUT75 的全部灯效模式，按官方 `sort` 字段排序（关闭背光排在最后）。
pub const EFFECT_MODES: &[EffectMode] = &[
    EffectMode { id: 1, name: "流光", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 2, name: "静态恒亮", show_speed: false, show_color: true, show_direction: false },
    EffectMode { id: 3, name: "动态呼吸", show_speed: true, show_color: true, show_direction: false },
    EffectMode { id: 4, name: "光谱循环", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 6, name: "灵动", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 7, name: "连锁反应", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 8, name: "拾音灯效1", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 9, name: "拾音灯效2", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 10, name: "拾音灯效3", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 11, name: "拾音灯效4", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 12, name: "跑马灯效", show_speed: true, show_color: false, show_direction: false },
    EffectMode { id: 23, name: "彩虹风车", show_speed: true, show_color: false, show_direction: true },
    EffectMode { id: 24, name: "七彩聚拢", show_speed: true, show_color: true, show_direction: false },
    EffectMode { id: 25, name: "霓虹叠影", show_speed: true, show_color: true, show_direction: true },
    EffectMode { id: 5, name: "关闭背光", show_speed: false, show_color: false, show_direction: false },
];

/// 按编号查模式。未知编号返回 `None`——不要在设备报了个没见过的值时瞎猜名字。
pub fn effect_mode(id: u8) -> Option<&'static EffectMode> {
    EFFECT_MODES.iter().find(|mode| mode.id == id)
}

/// 模式编号转中文名，未知时带上编号。
pub fn effect_mode_name(id: u8) -> String {
    match effect_mode(id) {
        Some(mode) => mode.name.to_string(),
        None => format!("未知模式 {id}"),
    }
}

/// 主灯效，`GET_LED_EFFECT` 0x13 / `SET_LED_EFFECT` 0x23，16 字节。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LedEffect {
    /// 读回来的原始字节，写到未建模位置的内容靠它保留。
    #[serde(skip_serializing)]
    raw: [u8; LED_EFFECT_LEN],
    /// 灯效模式。取值与名称的对照表官方没有静态暴露，需要用真机逐个试探。
    pub mode: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub driver_setting: u8,
    /// 次级颜色，双色灯效用。
    pub secondary_red: u8,
    pub secondary_green: u8,
    pub secondary_blue: u8,
    /// 单色 / 彩色。
    pub color_mode: u8,
    /// 亮度，NUT75 范围 1–6。
    pub brightness: u8,
    /// 速度，NUT75 范围 1–6。
    pub speed: u8,
    pub direction: u8,
    pub effect_mode_type: u8,
    /// 帧尾校验，读出来原样写回即可。
    pub check_code: [u8; 2],
}

impl LedEffect {
    pub const LEN: usize = LED_EFFECT_LEN;

    pub fn parse(data: &[u8]) -> Self {
        let mut raw = [0u8; Self::LEN];
        let len = data.len().min(Self::LEN);
        raw[..len].copy_from_slice(&data[..len]);
        Self {
            raw,
            mode: data[0],
            red: data[1],
            green: data[2],
            blue: data[3],
            driver_setting: data[4],
            secondary_red: data[5],
            secondary_green: data[6],
            secondary_blue: data[7],
            color_mode: data[8],
            brightness: data[9],
            speed: data[10],
            direction: data[11],
            effect_mode_type: data[12],
            check_code: [data[14], data[15]],
        }
    }

    /// 写回用的 16 字节：原始值打底，已建模字段覆盖上去。
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.raw;
        out[0] = self.mode;
        out[1] = self.red;
        out[2] = self.green;
        out[3] = self.blue;
        out[4] = self.driver_setting;
        out[5] = self.secondary_red;
        out[6] = self.secondary_green;
        out[7] = self.secondary_blue;
        out[8] = self.color_mode;
        out[9] = self.brightness;
        out[10] = self.speed;
        out[11] = self.direction;
        out[12] = self.effect_mode_type;
        out[14] = self.check_code[0];
        out[15] = self.check_code[1];
        out.to_vec()
    }

    /// 读回来的原始字节（诊断用）。
    pub fn raw(&self) -> [u8; Self::LEN] {
        self.raw
    }

    /// 只改一两个字段时用它，其余字节（含未建模的）原样保留。
    pub fn with(&self, f: impl FnOnce(&mut Self)) -> Self {
        let mut next = *self;
        f(&mut next);
        next
    }
}

/// 侧灯 / 灯箱，`GET_SIDE_LIGHT` 0x1D、`GET_LIGHT_BOX` 0x1B，都是 24 字节。
///
/// 两者字段完全一致，只是命令不同，所以共用一个类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SideLight {
    /// 读回来的原始字节，写到未建模位置的内容靠它保留。
    #[serde(skip_serializing)]
    raw: [u8; SIDE_LIGHT_LEN],
    pub mode: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub color_mode: u8,
    /// 亮度，1–6。
    pub brightness: u8,
    /// 速度，1–6。
    pub speed: u8,
}

impl SideLight {
    pub const LEN: usize = SIDE_LIGHT_LEN;

    pub fn parse(data: &[u8]) -> Self {
        let mut raw = [0u8; Self::LEN];
        let len = data.len().min(Self::LEN);
        raw[..len].copy_from_slice(&data[..len]);
        Self {
            raw,
            mode: data[0],
            red: data[1],
            green: data[2],
            blue: data[3],
            color_mode: data[8],
            brightness: data[9],
            speed: data[10],
        }
    }

    /// 写回用的 24 字节：原始值打底，已建模字段覆盖上去。
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.raw;
        out[0] = self.mode;
        out[1] = self.red;
        out[2] = self.green;
        out[3] = self.blue;
        out[8] = self.color_mode;
        out[9] = self.brightness;
        out[10] = self.speed;
        out.to_vec()
    }

    pub fn raw(&self) -> [u8; Self::LEN] {
        self.raw
    }

    pub fn with(&self, f: impl FnOnce(&mut Self)) -> Self {
        let mut next = *self;
        f(&mut next);
        next
    }
}

/// 单个 LED 的颜色。自定义灯效和逐键取色都用这个结构。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct LedColor {
    pub led_id: u8,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl LedColor {
    /// 一个 LED 占 4 字节。
    pub const SIZE: usize = 4;

    /// 解析 512 字节的颜色表（128 个 LED）。
    pub fn parse_all(data: &[u8]) -> Vec<Self> {
        data.chunks_exact(Self::SIZE)
            .map(|c| Self { led_id: c[0], red: c[1], green: c[2], blue: c[3] })
            .collect()
    }

    pub fn encode_all(colors: &[Self]) -> Vec<u8> {
        let mut out = Vec::with_capacity(colors.len() * Self::SIZE);
        for c in colors {
            out.extend_from_slice(&[c.led_id, c.red, c.green, c.blue]);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn led_effect_round_trips_and_keeps_check_bytes() {
        let mut raw = vec![0u8; 16];
        raw[0] = 5; // mode
        raw[1..4].copy_from_slice(&[255, 0, 128]);
        raw[9] = 4; // brightness
        raw[10] = 2; // speed
        raw[14] = 0xAA;
        raw[15] = 0x55;
        let effect = LedEffect::parse(&raw);
        assert_eq!(effect.mode, 5);
        assert_eq!((effect.red, effect.green, effect.blue), (255, 0, 128));
        assert_eq!(effect.brightness, 4);
        assert_eq!(effect.check_code, [0xAA, 0x55]);
        assert_eq!(effect.encode(), raw);
    }

    #[test]
    fn side_light_uses_offset_eight_for_color_mode() {
        let mut raw = vec![0u8; 24];
        raw[0] = 3;
        raw[1..4].copy_from_slice(&[10, 20, 30]);
        raw[8] = 1;
        raw[9] = 6;
        raw[10] = 5;
        let light = SideLight::parse(&raw);
        assert_eq!(light.mode, 3);
        assert_eq!((light.red, light.green, light.blue), (10, 20, 30));
        assert_eq!(light.color_mode, 1);
        assert_eq!(light.brightness, 6);
        assert_eq!(light.encode(), raw);
    }

    #[test]
    fn custom_led_lists_every_led() {
        let mut raw = vec![0u8; 512];
        for i in 0..128usize {
            raw[i * 4] = i as u8;
            raw[i * 4 + 1] = 1;
        }
        let colors = LedColor::parse_all(&raw);
        assert_eq!(colors.len(), 128);
        assert_eq!(colors[7].led_id, 7);
        assert_eq!(colors[7].red, 1);
        assert_eq!(LedColor::encode_all(&colors), raw);
    }

    #[test]
    fn unmodelled_bytes_survive_a_write_back() {
        // 灯效的 [13] 没有名字，但官方结构里它是校验的一部分。
        // 只改亮度，其余字节必须原样写回。
        let mut raw = vec![0u8; 16];
        raw[0] = 2; // mode
        raw[1..4].copy_from_slice(&[255, 255, 255]);
        raw[9] = 6; // brightness
        raw[10] = 6; // speed
        raw[13] = 0xC3; // 未建模
        raw[14] = 0xAA;
        raw[15] = 0x55;

        let next = LedEffect::parse(&raw).with(|e| e.brightness = 3);
        let written = next.encode();
        assert_eq!(written[9], 3, "改动的字段要生效");
        assert_eq!(written[13], 0xC3, "未建模字节不能被清零");
        assert_eq!(written[14..], [0xAA, 0x55], "校验位要保留");
    }

    #[test]
    fn side_light_keeps_the_padding_it_read() {
        // 侧灯只有 [0..4] 和 [8..11] 有名字，其余 15 个字节是保留区。
        let mut raw = vec![0u8; 24];
        raw[0] = 3;
        raw[1..4].copy_from_slice(&[10, 20, 30]);
        for i in [4, 5, 6, 7, 11, 12, 23] {
            raw[i] = 0x80 + i as u8;
        }
        let next = SideLight::parse(&raw).with(|s| s.speed = 2);
        let written = next.encode();
        assert_eq!(written[10], 2);
        for i in [4, 5, 6, 7, 11, 12, 23] {
            assert_eq!(written[i], 0x80 + i as u8, "保留位 {i} 被改了");
        }
    }

    #[test]
    fn effect_mode_table_matches_the_official_list() {
        // 15 个模式：12 个基础 + 3 个追加（23/24/25），关闭背光排最后。
        assert_eq!(EFFECT_MODES.len(), 15);
        assert_eq!(EFFECT_MODES.last().unwrap().name, "关闭背光");
        assert_eq!(effect_mode_name(2), "静态恒亮");
        assert_eq!(effect_mode_name(12), "跑马灯效");
        assert_eq!(effect_mode_name(23), "彩虹风车");
        // 真机读回来就是 2，指的就是静态恒亮
        assert_eq!(effect_mode(2).unwrap().name, "静态恒亮");
        // 没见过的编号不编名字
        assert_eq!(effect_mode_name(99), "未知模式 99");
        assert!(effect_mode(0).is_none());
    }

    #[test]
    fn effect_mode_ids_are_unique_and_flags_are_sane() {
        let mut ids: Vec<u8> = EFFECT_MODES.iter().map(|m| m.id).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count, "模式编号有重复");

        // 关闭背光既没有速度也没有颜色
        let off = effect_mode(5).unwrap();
        assert!(!off.show_speed && !off.show_color && !off.show_direction);
        // 静态恒亮没有速度但有颜色
        let solid = effect_mode(2).unwrap();
        assert!(!solid.show_speed && solid.show_color);
        // 彩虹风车能调方向
        assert!(effect_mode(23).unwrap().show_direction);
    }
}
