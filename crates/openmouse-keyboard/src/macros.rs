//! 宏：400 字节的索引表 + 从偏移 400 开始的数据区。
//!
//! 布局取自官方驱动 `getMacroData` / `setMacroData`：
//!
//! ```text
//! 索引表 400 B = 100 槽 × 4 B（u32 小端；0 = 空槽，否则是数据区偏移）
//! 每个宏的数据：
//!   [0..1] 字节长度，等于 动作数 × 2
//!   [3]    保留
//!   动作 4 B × n：
//!     [0..1] delay（毫秒）
//!     [2]    keyCode
//!     [3]    flags：高位 0x80 = 按下，否则抬起；
//!            (flags >> 4) & 7 = 动作类型，鼠标键是 0x90/0x10，键盘键是 0xB0/0x30
//! ```

/// 索引表的槽位数。
pub const SLOTS: usize = 100;
/// 索引表字节数（100 × 4）。
pub const INDEX_BYTES: usize = SLOTS * 4;
/// 数据区的起始偏移。
pub const DATA_OFFSET: u32 = INDEX_BYTES as u32;

/// 一个宏动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Action {
    /// 间隔 / 按住时长，毫秒。
    pub delay: u16,
    /// HID 键码。
    pub key_code: u8,
    pub flags: u8,
}

impl Action {
    pub const SIZE: usize = 4;

    /// 这一条是按下还是抬起。
    pub fn is_press(&self) -> bool {
        self.flags & 0x80 != 0
    }

    /// 动作类型（0–7）。鼠标键与键盘键在上字节上不同，`flags` 原值可判断。
    pub fn action_type(&self) -> u8 {
        (self.flags >> 4) & 0x07
    }

    /// 鼠标动作（0x90 / 0x10）还是键盘动作（0xB0 / 0x30）。
    pub fn is_mouse(&self) -> bool {
        self.flags & 0x20 == 0
    }

    pub fn parse(data: &[u8]) -> Self {
        Self {
            delay: u16::from_le_bytes([data[0], data[1]]),
            key_code: data[2],
            flags: data[3],
        }
    }

    pub fn encode(&self) -> [u8; Self::SIZE] {
        let [lo, hi] = self.delay.to_le_bytes();
        [lo, hi, self.key_code, self.flags]
    }

    /// 键盘按键动作。`flags` 官方是 `isPress ? 0xB0 : 0x30`。
    pub fn keyboard(key_code: u8, is_press: bool, delay: u16) -> Self {
        Self { delay, key_code, flags: if is_press { 0xB0 } else { 0x30 } }
    }

    /// 鼠标按键动作。`flags` 官方是 `isPress ? 0x90 : 0x10`。
    pub fn mouse(button: u8, is_press: bool, delay: u16) -> Self {
        Self { delay, key_code: button, flags: if is_press { 0x90 } else { 0x10 } }
    }

    /// 翻转按下/抬起，延时不变。
    pub fn toggled(&self) -> Self {
        let mut next = *self;
        next.flags ^= 0x80;
        next
    }
}

/// 一个宏槽位。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Macro {
    /// 0–99。
    pub id: u8,
    /// 数据区里的起始偏移，空槽为 0。
    pub address: u32,
    pub actions: Vec<Action>,
}

impl Macro {
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }
}

/// 解析索引表，返回 `(槽位, 数据区偏移)`，跳过空槽。
pub fn occupied_slots(index: &[u8]) -> Vec<(u8, u32)> {
    index
        .chunks_exact(4)
        .take(SLOTS)
        .enumerate()
        .filter_map(|(slot, bytes)| {
            let address = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            (address != 0).then_some((slot as u8, address))
        })
        .collect()
}

/// 从宏数据头解出动作数量：长度字段是 `动作数 × 2`。
pub fn action_count(header: &[u8]) -> usize {
    if header.len() < 2 {
        return 0;
    }
    let length = u16::from_le_bytes([header[0], header[1]]) as usize;
    length / 2
}

/// 把动作列表编成数据区格式（含 4 字节头）。
pub fn encode_actions(actions: &[Action]) -> Vec<u8> {
    let mut out = vec![0u8; 4 + actions.len() * Action::SIZE];
    let length = (actions.len() * 2) as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    for (i, action) in actions.iter().enumerate() {
        let at = 4 + i * Action::SIZE;
        out[at..at + Action::SIZE].copy_from_slice(&action.encode());
    }
    out
}

/// 把 `(槽位, 偏移)` 编回 400 字节索引表。
pub fn encode_index(slots: &[(u8, u32)]) -> Vec<u8> {
    let mut out = vec![0u8; INDEX_BYTES];
    for &(slot, address) in slots {
        if (slot as usize) < SLOTS {
            let at = slot as usize * 4;
            out[at..at + 4].copy_from_slice(&address.to_le_bytes());
        }
    }
    out
}

/// 把一组宏编成「索引表 + 数据区」两段，地址从 400 顺序排下去。
///
/// 官方驱动的 `setMacroData` 就是这个排布：空宏（没有动作）不占数据区，
/// 后面的宏依次紧挨着排。返回的索引表永远是 400 字节，数据区可能为空。
///
/// 注意顺序：**索引表先写、数据区后写**，且索引表那次不能带尾包标志。
pub fn encode_table(macros: &[Macro]) -> (Vec<u8>, Vec<u8>) {
    let mut index = vec![0u8; INDEX_BYTES];
    let mut data = Vec::new();
    for macro_data in macros {
        if macro_data.actions.is_empty() || macro_data.id as usize >= SLOTS {
            continue;
        }
        let address = DATA_OFFSET + data.len() as u32;
        let at = macro_data.id as usize * 4;
        index[at..at + 4].copy_from_slice(&address.to_le_bytes());
        data.extend_from_slice(&encode_actions(&macro_data.actions));
    }
    (index, data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_table_skips_empty_slots() {
        let mut index = vec![0u8; INDEX_BYTES];
        index[0..4].copy_from_slice(&400u32.to_le_bytes()); // 槽 0 → 偏移 400
        index[4 * 3..4 * 3 + 4].copy_from_slice(&412u32.to_le_bytes()); // 槽 3
        let slots = occupied_slots(&index);
        assert_eq!(slots, vec![(0, 400), (3, 412)]);
    }

    #[test]
    fn action_count_is_half_the_length_field() {
        // 3 个动作 → 长度字段 6
        assert_eq!(action_count(&[6, 0, 0, 0]), 3);
        assert_eq!(action_count(&[0, 0, 0, 0]), 0);
        assert_eq!(action_count(&[]), 0);
    }

    #[test]
    fn encodes_and_decodes_a_macro() {
        let actions = vec![
            Action { delay: 10, key_code: 0x04, flags: 0xB0 }, // A 按下
            Action { delay: 20, key_code: 0x04, flags: 0x30 }, // A 抬起
            Action { delay: 5, key_code: 0x05, flags: 0x90 },  // 鼠标键按下
        ];
        let data = encode_actions(&actions);
        assert_eq!(&data[..2], &[6, 0]);
        assert_eq!(action_count(&data), 3);
        let parsed: Vec<Action> = data[4..].chunks_exact(4).map(Action::parse).collect();
        assert_eq!(parsed, actions);
        assert!(parsed[0].is_press());
        assert!(!parsed[1].is_press());
        assert!(parsed[2].is_mouse());
        assert!(!parsed[0].is_mouse());
    }

    #[test]
    fn index_round_trips() {
        let slots = vec![(0u8, 400u32), (3, 412)];
        assert_eq!(occupied_slots(&encode_index(&slots)), slots);
    }

    #[test]
    fn table_packs_macros_back_to_back_from_400() {
        // 槽 0 两个动作（4 + 8 = 12 字节），槽 2 一个动作（4 + 4 = 8 字节）
        let a = Macro {
            id: 0,
            address: 0,
            actions: vec![Action::keyboard(0x04, true, 10), Action::keyboard(0x04, false, 20)],
        };
        let b = Macro { id: 2, address: 0, actions: vec![Action::mouse(1, true, 5)] };
        let (index, data) = encode_table(&[a, b]);
        assert_eq!(index.len(), INDEX_BYTES);
        assert_eq!(data.len(), 20);
        assert_eq!(occupied_slots(&index), vec![(0, 400), (2, 412)]);
        // 头两个字节是 动作数 × 2
        assert_eq!(&data[..2], &[4, 0]);
        assert_eq!(&data[12..14], &[2, 0]);
    }

    #[test]
    fn empty_macros_take_no_space() {
        let empty = Macro { id: 1, address: 0, actions: Vec::new() };
        let real = Macro { id: 5, address: 0, actions: vec![Action::keyboard(0x05, true, 1)] };
        let (index, data) = encode_table(&[empty, real]);
        assert_eq!(occupied_slots(&index), vec![(5, 400)]);
        assert_eq!(data.len(), 8);
        // 空槽不写地址，设备上就是"没有这个宏"
        assert_eq!(&index[4..8], &[0, 0, 0, 0]);
    }

    #[test]
    fn action_helpers_pick_the_official_flags() {
        let press = Action::keyboard(0x04, true, 10);
        let release = press.toggled();
        assert_eq!(press.flags, 0xB0);
        assert_eq!(release.flags, 0x30);
        assert!(!press.is_mouse() && !release.is_mouse());
        let mouse = Action::mouse(2, true, 0);
        assert_eq!(mouse.flags, 0x90);
        assert!(mouse.is_mouse());
        assert_eq!(mouse.toggled().flags, 0x10);
    }
}
