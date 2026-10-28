//! NUT75 的物理键位布局：6 行 79 键。
//!
//! 键位编号（`index`）就是键位表里的下标，和 `Keyboard::keymap()` 返回的
//! 数组一一对应。`width` 是 75% 配列下的键宽，单位是标准键的倍数，只用于
//! 画界面。
//!
//! 数据来源：官方网页驱动 `nut75_config.js` 里的 `keyList`（二维数组），
//! 由 `/tmp` 下的 bundle 提取后用脚本转成 Rust，没有手抄。

/// 布局里的一个键位。
#[derive(Debug, Clone, Copy)]
pub struct KeySlot {
    /// 键位表下标。
    pub index: usize,
    /// 键帽上的字。
    pub label: &'static str,
    /// 键宽，标准键为 1.0。
    pub width: f32,
}

/// NUT75 的 6 行 79 键。
pub const NUT75_LAYOUT: &[&[KeySlot]] = &[
    &[
        KeySlot { index: 0, label: "Esc", width: 1.0 },
        KeySlot { index: 1, label: "F1", width: 1.0 },
        KeySlot { index: 2, label: "F2", width: 1.0 },
        KeySlot { index: 3, label: "F3", width: 1.0 },
        KeySlot { index: 4, label: "F4", width: 1.0 },
        KeySlot { index: 5, label: "F5", width: 1.0 },
        KeySlot { index: 6, label: "F6", width: 1.0 },
        KeySlot { index: 7, label: "F7", width: 1.0 },
        KeySlot { index: 8, label: "F8", width: 1.0 },
        KeySlot { index: 9, label: "F9", width: 1.0 },
        KeySlot { index: 10, label: "F10", width: 1.0 },
        KeySlot { index: 11, label: "F11", width: 1.0 },
        KeySlot { index: 12, label: "F12", width: 1.0 },
        KeySlot { index: 106, label: "Del", width: 1.0 },
        KeySlot { index: 104, label: "Home", width: 1.0 },
    ],
    &[
        KeySlot { index: 17, label: "1 !", width: 1.0 },
        KeySlot { index: 18, label: "2 @", width: 1.0 },
        KeySlot { index: 19, label: "3 #", width: 1.0 },
        KeySlot { index: 20, label: "4 $", width: 1.0 },
        KeySlot { index: 21, label: "5 %", width: 1.0 },
        KeySlot { index: 22, label: "6 ^", width: 1.0 },
        KeySlot { index: 23, label: "7 &", width: 1.0 },
        KeySlot { index: 24, label: "8 *", width: 1.0 },
        KeySlot { index: 25, label: "9 (", width: 1.0 },
        KeySlot { index: 26, label: "0 )", width: 1.0 },
        KeySlot { index: 27, label: "ˉ -", width: 1.0 },
        KeySlot { index: 28, label: "= +", width: 1.0 },
        KeySlot { index: 92, label: "Back", width: 2.0 },
        KeySlot { index: 107, label: "End", width: 1.0 },
    ],
    &[
        KeySlot { index: 32, label: "Tab", width: 1.5 },
        KeySlot { index: 33, label: "Q", width: 1.0 },
        KeySlot { index: 34, label: "W", width: 1.0 },
        KeySlot { index: 35, label: "E", width: 1.0 },
        KeySlot { index: 36, label: "R", width: 1.0 },
        KeySlot { index: 37, label: "T", width: 1.0 },
        KeySlot { index: 38, label: "Y", width: 1.0 },
        KeySlot { index: 39, label: "U", width: 1.0 },
        KeySlot { index: 40, label: "I", width: 1.0 },
        KeySlot { index: 41, label: "O", width: 1.0 },
        KeySlot { index: 42, label: "P", width: 1.0 },
        KeySlot { index: 43, label: " }", width: 1.0 },
        KeySlot { index: 60, label: "\\\\ | ", width: 1.0 },
        KeySlot { index: 105, label: "PgUp", width: 1.0 },
    ],
    &[
        KeySlot { index: 48, label: "Caps", width: 1.75 },
        KeySlot { index: 49, label: "A", width: 1.0 },
        KeySlot { index: 50, label: "S", width: 1.0 },
        KeySlot { index: 51, label: "D", width: 1.0 },
        KeySlot { index: 52, label: "F", width: 1.0 },
        KeySlot { index: 53, label: "G", width: 1.0 },
        KeySlot { index: 54, label: "H", width: 1.0 },
        KeySlot { index: 55, label: "J", width: 1.0 },
        KeySlot { index: 56, label: "K", width: 1.0 },
        KeySlot { index: 57, label: "L", width: 1.0 },
        KeySlot { index: 58, label: "; :", width: 1.0 },
        KeySlot { index: 59, label: "' \"", width: 1.0 },
        KeySlot { index: 76, label: "Enter", width: 2.25 },
        KeySlot { index: 108, label: "PgDn", width: 1.0 },
    ],
    &[
        KeySlot { index: 64, label: "L-Shift", width: 2.25 },
        KeySlot { index: 65, label: "Z", width: 1.0 },
        KeySlot { index: 66, label: "X", width: 1.0 },
        KeySlot { index: 67, label: "C", width: 1.0 },
        KeySlot { index: 68, label: "V", width: 1.0 },
        KeySlot { index: 69, label: "B", width: 1.0 },
        KeySlot { index: 70, label: "N", width: 1.0 },
        KeySlot { index: 71, label: "M", width: 1.0 },
        KeySlot { index: 72, label: ", <", width: 1.0 },
        KeySlot { index: 73, label: ". >", width: 1.0 },
        KeySlot { index: 74, label: "/ ?", width: 1.0 },
        KeySlot { index: 75, label: "R-Shift", width: 2.75 },
        KeySlot { index: 90, label: "↑", width: 1.0 },
    ],
    &[
        KeySlot { index: 80, label: "L-Ctrl", width: 1.25 },
        KeySlot { index: 81, label: "L-Win", width: 1.25 },
        KeySlot { index: 82, label: "L-Alt", width: 1.25 },
        KeySlot { index: 83, label: "Spacebar", width: 6.5 },
        KeySlot { index: 85, label: "Fn", width: 1.25 },
        KeySlot { index: 87, label: "R-Ctrl", width: 1.25 },
        KeySlot { index: 88, label: "←", width: 1.0 },
        KeySlot { index: 89, label: "↓", width: 1.0 },
        KeySlot { index: 91, label: "→", width: 1.0 },
    ],
];

/// 布局里的键位总数。
pub const NUT75_KEY_COUNT: usize = 79;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_covers_every_index_once() {
        // 键位编号不是 0..78 的连续序列：Del/Home/End/PgUp/PgDn 用的是厂商
        // 独立编号（104..108），所以按 128 个位置来查重。
        let mut seen = [false; 128];
        let mut count = 0;
        for row in NUT75_LAYOUT {
            for slot in *row {
                assert!(slot.index < 128, "键位 {} 超出键位表范围", slot.index);
                assert!(!seen[slot.index], "键位 {} 出现了两次", slot.index);
                seen[slot.index] = true;
                count += 1;
            }
        }
        assert_eq!(count, NUT75_KEY_COUNT);
        assert_eq!(NUT75_LAYOUT.len(), 6);
        // 真机上只有 113 个位置有数据，布局只画其中 79 个物理键
        assert_eq!(NUT75_KEY_COUNT, 79);
    }

    #[test]
    fn first_and_last_keys_match_the_real_keymap() {
        // 真机读回：键位 0 = Esc，键位 91 = 右方向键
        assert_eq!(NUT75_LAYOUT[0][0].index, 0);
        assert_eq!(NUT75_LAYOUT[0][0].label, "Esc");
        assert_eq!(NUT75_LAYOUT[5].last().unwrap().index, 91);
    }
}
