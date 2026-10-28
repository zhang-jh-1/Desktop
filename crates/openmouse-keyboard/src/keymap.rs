//! 键位编码：每个键 4 字节，`[page, param1, param2, param3]`。
//!
//! `page` 决定另外三个字节怎么读。规则取自官方网页驱动的 `Eq()`，已在真机
//! 读回的 512 字节上验证（键位 0 = Esc，1..12 = F1..F12，13..15 = 音量组）。
//!
//! 这个模块保持**无损**：内部只存 4 个原始字节，解析出的 `Page` 都是即时计算
//! 的。这样读出来的数据原样写回去，不会因为解码/再编码丢掉厂商的私有位。

/// 键位的页面类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// 未自定义，沿用出厂键位。
    Default,
    Mouse,
    Keyboard,
    /// 多媒体键（音量、播放控制等），值是 16 位 Consumer usage。
    Consumer,
    /// 系统键（关机、睡眠等）。
    System,
    ExtraFunction,
    Macro,
    Cb,
    /// DKS：一次按下触发两次。
    Dks,
    /// MT：短按/长按不同功能。
    Mt,
    /// Toggle：按一次切换一次。
    Tgl,
    /// SOCD：同时按两个方向键时的优先级。
    Socd,
    Rs,
    Func,
    End,
    Mpt,
    /// 高位为 1 的扩展页面（官方 `FUNC_V2`）。
    FuncV2,
    Unknown(u8),
}

impl Page {
    pub fn from_byte(byte: u8) -> Self {
        // 0x80 位表示扩展页面格式（官方 FUNC_V2 的 `128 | …`）
        if byte & 0x80 != 0 {
            return Self::FuncV2;
        }
        match byte {
            0 => Self::Default,
            1 => Self::Mouse,
            2 => Self::Keyboard,
            3 => Self::Consumer,
            4 => Self::System,
            5 => Self::ExtraFunction,
            6 => Self::Macro,
            7 => Self::Cb,
            8 => Self::Dks,
            9 => Self::Mt,
            10 => Self::Tgl,
            11 => Self::Socd,
            12 => Self::Rs,
            13 => Self::Func,
            14 => Self::End,
            15 => Self::Mpt,
            other => Self::Unknown(other),
        }
    }

    /// 这个页面类型是否带可显示的功能参数。
    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "默认",
            Self::Mouse => "鼠标",
            Self::Keyboard => "键盘",
            Self::Consumer => "多媒体",
            Self::System => "系统",
            Self::ExtraFunction => "扩展功能",
            Self::Macro => "宏",
            Self::Cb => "CB",
            Self::Dks => "DKS",
            Self::Mt => "MT",
            Self::Tgl => "TGL",
            Self::Socd => "SOCD",
            Self::Rs => "RS",
            Self::Func => "FUNC",
            Self::End => "END",
            Self::Mpt => "MPT",
            Self::FuncV2 => "扩展",
            Self::Unknown(_) => "未知",
        }
    }
}

/// 一个键位的 4 字节原始数据。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
pub struct KeyBinding(pub [u8; 4]);

impl KeyBinding {
    pub fn page(&self) -> Page {
        Page::from_byte(self.0[0])
    }

    /// 是否为出厂默认（全 0）。
    pub fn is_default(&self) -> bool {
        self.0 == [0, 0, 0, 0]
    }

    pub fn raw(&self) -> [u8; 4] {
        self.0
    }

    /// 键盘页面下的 HID usage（如 0x29 = Esc）。
    pub fn keyboard_usage(&self) -> Option<u8> {
        (self.page() == Page::Keyboard).then_some(self.0[2])
    }

    /// 多媒体页面下的 16 位 Consumer usage（如 0x00E9 = 音量加）。
    pub fn consumer_usage(&self) -> Option<u16> {
        (self.page() == Page::Consumer).then(|| u16::from_le_bytes([self.0[1], self.0[2]]))
    }

    /// 鼠标页面下的按键号。
    pub fn mouse_button(&self) -> Option<u8> {
        (self.page() == Page::Mouse).then_some(self.0[1])
    }

    /// 宏页面引用的宏序号。
    pub fn macro_index(&self) -> Option<u8> {
        (self.page() == Page::Macro).then_some(self.0[1])
    }

    /// 给 UI 用的一行中文描述。
    pub fn describe(&self) -> String {
        match self.page() {
            Page::Default => "出厂默认".to_string(),
            Page::Keyboard => format!("键码 0x{:02X}", self.0[2]),
            Page::Consumer => format!("多媒体 0x{:04X}", u16::from_le_bytes([self.0[1], self.0[2]])),
            Page::Mouse => format!("鼠标键 {}", self.0[1]),
            Page::Macro => format!("宏 #{}", self.0[1]),
            other => format!("{} [{:02X} {:02X} {:02X}]", other.label(), self.0[1], self.0[2], self.0[3]),
        }
    }
}

// ---- 构造（写入时用；与官方 Eq() 的布局一致）----

/// 键盘按键：`[2, 0, usage, 0]`
pub fn keyboard_key(usage: u8) -> KeyBinding {
    KeyBinding([2, 0, usage, 0])
}

/// 多媒体键：`[3, usage_lo, usage_hi, 0]`
pub fn consumer_key(usage: u16) -> KeyBinding {
    let [lo, hi] = usage.to_le_bytes();
    KeyBinding([3, lo, hi, 0])
}

/// 鼠标键：`[1, 按键号, 参数, 0]`
pub fn mouse_key(button: u8) -> KeyBinding {
    KeyBinding([1, button.max(1), 0, 0])
}

/// 宏：`[6, 宏序号, 参数, 参数]`
pub fn macro_key(index: u8) -> KeyBinding {
    KeyBinding([6, index, 0, 0])
}

/// SOCD：`[11, 参数1, 参数2, 参数3]`
pub fn socd_key(param1: u8, param2: u8) -> KeyBinding {
    KeyBinding([11, param1, param2, 0])
}

/// 恢复出厂：`[0, 0, 0, 0]`
pub fn factory_default() -> KeyBinding {
    KeyBinding([0, 0, 0, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_the_keymap_read_from_real_hardware() {
        // 真机读回的原始字节（GET_KEY 的前几个键）
        let keys: [[u8; 4]; 4] = [
            [2, 0, 0x29, 0], // 键位 0 = Esc
            [2, 0, 0x3A, 0], // 键位 1 = F1
            [3, 0xE9, 0x00, 0], // 键位 13 = 音量加
            [0, 0, 0, 0],    // 未改
        ];
        assert_eq!(KeyBinding(keys[0]).keyboard_usage(), Some(0x29));
        assert_eq!(KeyBinding(keys[1]).keyboard_usage(), Some(0x3A));
        assert_eq!(KeyBinding(keys[2]).consumer_usage(), Some(0x00E9));
        assert!(KeyBinding(keys[3]).is_default());
    }

    #[test]
    fn encoders_round_trip_through_the_decoder() {
        assert_eq!(keyboard_key(0x29).keyboard_usage(), Some(0x29));
        assert_eq!(consumer_key(0x00E9).consumer_usage(), Some(0x00E9));
        assert_eq!(mouse_key(2).mouse_button(), Some(2));
        assert_eq!(macro_key(7).macro_index(), Some(7));
        assert_eq!(factory_default(), KeyBinding([0, 0, 0, 0]));
    }

    #[test]
    fn page_byte_recognises_extended_pages() {
        assert_eq!(Page::from_byte(0), Page::Default);
        assert_eq!(Page::from_byte(2), Page::Keyboard);
        assert_eq!(Page::from_byte(11), Page::Socd);
        assert_eq!(Page::from_byte(0x81), Page::FuncV2); // 高位标记的扩展页
        assert_eq!(Page::from_byte(0x42), Page::Unknown(0x42));
    }

    #[test]
    fn unknown_pages_survive_a_round_trip() {
        // 厂商私有页面不能被改写丢内容
        let raw = [0x42, 0x11, 0x22, 0x33];
        assert_eq!(KeyBinding(raw).raw(), raw);
    }
}
