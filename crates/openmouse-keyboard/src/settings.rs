//! 键盘设置：整个「设置」页面就是一条 `GAME_MODE`，56 字节。
//!
//! 字段偏移取自官方驱动 `getGameMode`。NUT75 只用到其中一部分
//! （回报率、按键延时、休眠、Fn 层开关、NKRO），其余是给 TFT 屏 / 磁轴型号的。
//!
//! 56 字节里只有二十来个位置有名字，剩下的是厂内保留。写回时以读回的原始
//! 字节打底，这些保留位原样保留——拿全零重建再把结构写回去，等于把厂内参数
//! 全清了。

const GAME_MODE_LEN: usize = 56;

/// `GET_GAME_MODE` 0x11 / `SET_GAME_MODE` 0x21，56 字节。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct GameMode {
    /// 读回来的原始字节，没建模的位置靠它保留。
    #[serde(skip_serializing)]
    raw: [u8; GAME_MODE_LEN],
    pub game_mode: u8,
    /// Fn 层开关。
    pub fn_switch: u8,
    /// 休眠时间档位。
    pub sleep_time: u8,
    pub key_delay: u8,
    /// 有线回报率。**2.4G 走 [`wireless_report_rate`]，改这个没反应。**
    pub report_rate: u8,
    pub system_mode: u8,
    pub tft_display_time: u8,
    /// 触发死区，设备里是 ×100 的整数。
    pub top_dead_zone: u8,
    pub bottom_dead_zone: u8,
    pub stability_mode: u8,
    pub auto_calibration: u8,
    pub single_key_wakeup: u8,
    pub push_button_mode: u8,
    pub nkro_switch: u8,
    /// 2.4G 回报率。
    pub wireless_report_rate: u16,
    pub power_mode: u8,
}

impl GameMode {
    pub const LEN: usize = GAME_MODE_LEN;

    /// NUT75 配置里给出的可选回报率。
    pub const REPORT_RATES: [u32; 3] = [1000, 4000, 8000];

    /// 回报率**码值表**：设备存的是档位编号，不是 Hz。
    ///
    /// 取自官方驱动的 `reportRateList` 渲染表：
    /// `[{1000, 3}, {2000, 4}, {4000, 5}, {8000, 6}]`，再按机型的
    /// `reportRateList` 过滤。NUT75 是 `[1000, 4000, 8000]`，所以只保留
    /// 3 / 5 / 6 三档。设备实际报 `[5] = 6`，即 8000 Hz。
    pub const REPORT_RATE_CODES: [(u8, u32); 4] = [(3, 1000), (4, 2000), (5, 4000), (6, 8000)];

    /// 码值转频率。未知码值返回 `None`（不要瞎猜，宁可显示"未知"）。
    pub fn report_rate_hz(code: u8) -> Option<u32> {
        Self::REPORT_RATE_CODES.iter().find(|(c, _)| *c == code).map(|(_, hz)| *hz)
    }

    /// 频率转码值。不在表里返回 `None`。
    pub fn report_rate_code(hz: u32) -> Option<u8> {
        Self::REPORT_RATE_CODES.iter().find(|(_, h)| *h == hz).map(|(c, _)| *c)
    }

    /// NUT75 上这个字段实际会显示哪一档（过滤掉不属于本机型的档位）。
    pub fn report_rate_hz_for_nut75(&self) -> Option<u32> {
        let hz = Self::report_rate_hz(self.report_rate)?;
        Self::REPORT_RATES.contains(&hz).then_some(hz)
    }

    /// 当前真正生效的回报率。
    ///
    /// `wireless_report_rate` 非 0 时以它为准——那个字段存的是 **Hz**（8000），
    /// 而 `report_rate` 存的是档位码（6）。两个字段语义不同，别混。
    pub fn active_report_rate_hz(&self) -> Option<u32> {
        if self.wireless_report_rate != 0 {
            return Some(self.wireless_report_rate as u32);
        }
        self.report_rate_hz_for_nut75()
    }

    /// 改回报率，字段按官方驱动的规则挑：
    ///
    /// - 2.4G 且无线字段已经是非 0（说明这台设备用 Hz 记）→ 写 `wireless_report_rate`（Hz）
    /// - 其余情况 → 写 `report_rate`（档位码）
    ///
    /// 这是照抄官方 `handleSaveKeyboardSettings` 里的分支，不是自己发明的规则。
    pub fn with_report_rate(&self, hz: u32, is_wireless: bool) -> Self {
        let mut next = *self;
        if is_wireless && next.wireless_report_rate != 0 {
            next.wireless_report_rate = hz.min(u16::MAX as u32) as u16;
        } else if Self::REPORT_RATES.contains(&hz) {
            // 只认 NUT75 自己那三档；2000 Hz 在码表里有，但本机型没有这一档
            if let Some(code) = Self::report_rate_code(hz) {
                next.report_rate = code;
            }
        }
        next
    }

    pub fn parse(data: &[u8]) -> Self {
        let mut raw = [0u8; Self::LEN];
        let len = data.len().min(Self::LEN);
        raw[..len].copy_from_slice(&data[..len]);
        Self {
            raw,
            game_mode: data[1],
            fn_switch: data[2],
            sleep_time: data[3],
            key_delay: data[4],
            report_rate: data[5],
            system_mode: data[6],
            tft_display_time: data[7],
            top_dead_zone: data[8],
            bottom_dead_zone: data[9],
            stability_mode: data[11],
            auto_calibration: data[14],
            single_key_wakeup: data[15],
            push_button_mode: data[16],
            nkro_switch: data[17],
            wireless_report_rate: u16::from_le_bytes([data[18], data[19]]),
            power_mode: data[20],
        }
    }

    /// 写回用的 56 字节：原始值打底，已建模字段覆盖上去。
    pub fn encode(&self) -> Vec<u8> {
        let mut out = self.raw;
        out[1] = self.game_mode;
        out[2] = self.fn_switch;
        out[3] = self.sleep_time;
        out[4] = self.key_delay;
        out[5] = self.report_rate;
        out[6] = self.system_mode;
        out[7] = self.tft_display_time;
        out[8] = self.top_dead_zone;
        out[9] = self.bottom_dead_zone;
        out[11] = self.stability_mode;
        out[14] = self.auto_calibration;
        out[15] = self.single_key_wakeup;
        out[16] = self.push_button_mode;
        out[17] = self.nkro_switch;
        out[18..20].copy_from_slice(&self.wireless_report_rate.to_le_bytes());
        out[20] = self.power_mode;
        out.to_vec()
    }

    pub fn raw(&self) -> [u8; Self::LEN] {
        self.raw
    }

    /// 只改一两个字段时用它，其余字节（含没建模的）原样保留。
    pub fn with(&self, f: impl FnOnce(&mut Self)) -> Self {
        let mut next = *self;
        f(&mut next);
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings_at_the_right_offsets() {
        let mut raw = vec![0u8; GameMode::LEN];
        raw[1] = 1; // gameMode
        raw[2] = 1; // fnSwitch
        raw[3] = 5; // sleepTime
        raw[4] = 3; // keyDelay
        raw[5] = 3; // reportRate
        raw[17] = 1; // nkro
        raw[18..20].copy_from_slice(&8000u16.to_le_bytes());
        let settings = GameMode::parse(&raw);
        assert_eq!(settings.game_mode, 1);
        assert_eq!(settings.sleep_time, 5);
        assert_eq!(settings.key_delay, 3);
        assert_eq!(settings.report_rate, 3);
        assert_eq!(settings.nkro_switch, 1);
        assert_eq!(settings.wireless_report_rate, 8000);
        assert_eq!(settings.encode(), raw);
    }

    #[test]
    fn off_by_one_fields_do_not_leak_into_each_other() {
        // topDeadZone 在 [8]，bottomDeadZone 在 [9]，中间没有跳位
        let mut raw = vec![0u8; GameMode::LEN];
        raw[8] = 20;
        raw[9] = 35;
        let settings = GameMode::parse(&raw);
        assert_eq!(settings.top_dead_zone, 20);
        assert_eq!(settings.bottom_dead_zone, 35);
        // stabilityMode 在 [11]，[10] 不属于任何字段
        raw[10] = 99;
        assert_eq!(GameMode::parse(&raw).stability_mode, 0);
    }

    #[test]
    fn report_rate_is_a_code_not_a_frequency() {
        // 真机读回 [5] = 6，按官方表就是 8000 Hz。
        let mut raw = vec![0u8; GameMode::LEN];
        raw[5] = 6;
        let settings = GameMode::parse(&raw);
        assert_eq!(settings.report_rate, 6);
        assert_eq!(GameMode::report_rate_hz(6), Some(8000));
        assert_eq!(settings.report_rate_hz_for_nut75(), Some(8000));

        // 2000 Hz 的码值 4 合法，但 NUT75 没有这一档
        assert_eq!(GameMode::report_rate_hz(4), Some(2000));
        let mut raw2 = vec![0u8; GameMode::LEN];
        raw2[5] = 4;
        assert_eq!(GameMode::parse(&raw2).report_rate_hz_for_nut75(), None);

        assert_eq!(GameMode::report_rate_code(4000), Some(5));
        assert_eq!(GameMode::report_rate_code(2000), Some(4));
        assert_eq!(GameMode::report_rate_hz(9), None);
    }

    #[test]
    fn unmodelled_bytes_survive_a_write_back() {
        // 设备里有些位置没有名字，但可能被厂内固件用到。
        // 走一遍 parse → 改一个字段 → encode，其余字节必须一个不差。
        let mut raw = vec![0u8; GameMode::LEN];
        raw[0] = 0x11; // 未建模
        raw[10] = 0x22; // 未建模（夹在 deadZone 和 stabilityMode 之间）
        raw[12] = 0x33;
        raw[13] = 0x44;
        for i in 21..GameMode::LEN {
            raw[i] = i as u8; // 尾部保留区
        }
        let settings = GameMode::parse(&raw).with(|s| s.nkro_switch = 1);
        let written = settings.encode();
        assert_eq!(written[17], 1, "改动的字段要生效");
        assert_eq!(written[0], 0x11);
        assert_eq!(written[10], 0x22);
        assert_eq!(written[12], 0x33);
        assert_eq!(written[13], 0x44);
        assert_eq!(&written[21..], &raw[21..], "尾部保留区不能被清零");
    }

    #[test]
    fn report_rate_writes_the_field_the_link_actually_uses() {
        // 有线 / 无线字段还是 0 的机器：写档位码
        let mut wired = vec![0u8; GameMode::LEN];
        wired[5] = 6;
        let next = GameMode::parse(&wired).with_report_rate(4000, false);
        assert_eq!(next.report_rate, 5);
        assert_eq!(next.report_rate_hz_for_nut75(), Some(4000));
        assert_eq!(next.wireless_report_rate, 0, "有线不该动无线字段");

        // 2.4G 且无线字段已经用 Hz 记：写 Hz，档位码不动
        let mut wireless = vec![0u8; GameMode::LEN];
        wireless[5] = 6;
        wireless[18..20].copy_from_slice(&8000u16.to_le_bytes());
        let next = GameMode::parse(&wireless).with_report_rate(1000, true);
        assert_eq!(next.wireless_report_rate, 1000);
        assert_eq!(next.report_rate, 6, "无线模式下不该改有线档位码");
        assert_eq!(next.active_report_rate_hz(), Some(1000));

        // 本机现在的状态：2.4G + 无线字段 0 → 按有线那条规则写码值
        let mut current = vec![0u8; GameMode::LEN];
        current[5] = 6;
        let next = GameMode::parse(&current).with_report_rate(1000, true);
        assert_eq!(next.report_rate, 3);
        assert_eq!(next.active_report_rate_hz(), Some(1000));

        // 不认识的频率不写，别把字段写成 0
        let next = GameMode::parse(&current).with_report_rate(2000, false);
        assert_eq!(next.report_rate, 6, "NUT75 没有 2000 Hz 这一档");
    }
}
