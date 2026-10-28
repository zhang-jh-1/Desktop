//! 键盘支持：维咖 NUT75（Weikav，Sonix 方案）。
//!
//! 这是**键盘自己的包**，和鼠标那条线（`openmouse-hid` + `@openmouse/protocol`
//! + `openmouse-mouse`）互不引用。共用的是 `hidapi` 这层系统调用，不是代码。
//!
//! 协议来源是官方网页驱动的前端实现（`https://weikav.driveall.cn/`，明文 JS），
//! 已在真机（NUT75，固件 v1.45，2.4G 模式）验证：能读到设备信息与完整键位表。
//!
//! ```no_run
//! let kb = openmouse_keyboard::Keyboard::open()?;
//! println!("{:?}", kb.info());
//! for (i, key) in kb.keymap()?.iter().enumerate() {
//!     if !key.is_default() {
//!         println!("键位 {i}: {}", key.describe());
//!     }
//! }
//! # Ok::<(), openmouse_keyboard::Error>(())
//! ```

pub mod keymap;
pub mod layout;
pub mod lighting;
pub mod macros;
pub mod protocol;
pub mod settings;

pub use keymap::{KeyBinding, Page};
pub use layout::{KeySlot, NUT75_KEY_COUNT, NUT75_LAYOUT};
pub use lighting::{effect_mode, effect_mode_name, EffectMode, LedColor, LedEffect, SideLight, EFFECT_MODES};
pub use macros::{Action, Macro};
pub use settings::GameMode;
use protocol::cmd;

use serde::Serialize;

/// Sonix（松翰）的 USB VID。
pub const SONIX_VID: u16 = 0x0C45;
/// NUT75 有线模式的 PID。
pub const NUT75_WIRED_PID: u16 = 0x880C;
/// NUT75 的 2.4G 接收器 PID（产品名显示为 "NUT75 2.4G"）。
pub const NUT75_RECEIVER_PID: u16 = 0xFEF9;
/// NUT75 在 `GET_DEVICE_INFO` 里自报的产品 ID。
pub const NUT75_PRODUCT_ID: u16 = 0x001C;

/// 厂商自定义 usage page。实测配置通道走 `0xFF60`；
/// 官方 JS 里写的 `0xFF67` 对 NUT75 是错的，`0xFF70` 也不应答。
const CONFIG_USAGE_PAGES: [u16; 3] = [0xFF60, 0xFF67, 0xFF70];

/// 读一次应答的等待上限。
const RESPONSE_TIMEOUT_MS: i32 = 400;
/// 单条命令的重试次数。
const MAX_RETRIES: usize = 3;

#[derive(Debug)]
pub enum Error {
    /// 没找到键盘（未插、或处于蓝牙模式）。
    NoDevice,
    /// 找得到设备但打不开，通常是驱动通道被别的程序占用。
    Open(String),
    /// 收发失败。
    Io(String),
    /// 设备没在预期时间内应答。
    Timeout { cmd: u8 },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoDevice => write!(f, "未找到键盘：请确认已用 USB 线或 2.4G 接收器连接（蓝牙模式不提供驱动通道）"),
            Self::Open(reason) => write!(f, "打开键盘失败：{reason}"),
            Self::Io(reason) => write!(f, "键盘通信失败：{reason}"),
            Self::Timeout { cmd } => write!(f, "命令 0x{cmd:02X} 无应答"),
        }
    }
}

impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// `GET_DEVICE_INFO` 解出来的设备信息。
#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub rom_size: u8,
    /// 宏可用的字节数。
    pub macro_space: u16,
    /// 键盘自报的 VID（走 2.4G 时依然是 0x0C45）。
    pub vid: u16,
    /// 键盘自报的 PID（走 2.4G 时依然是有线 PID 0x880C）。
    pub pid: u16,
    /// 固件版本，如 1.45。
    pub firmware: f32,
    pub sensor: u16,
    pub vendor_id: u16,
    /// 型号 ID：NUT75 是 0x001C。
    pub product_id: u16,
    /// 0 有线 / 2 是 2.4G（实测 2.4G 为 2）。
    pub work_mode: u8,
    /// 电量百分比。
    pub battery: u8,
    pub charging: u8,
    pub profile: u8,
}

impl DeviceInfo {
    /// 当前连接方式的中文描述。
    pub fn connection(&self) -> &'static str {
        match self.work_mode {
            0 => "有线",
            2 => "2.4G",
            3 => "蓝牙",
            _ => "未知",
        }
    }

    /// 是否识别为 NUT75。
    pub fn is_nut75(&self) -> bool {
        self.product_id == NUT75_PRODUCT_ID
    }

    fn parse(data: &[u8]) -> Self {
        let u16_at = |i: usize| u16::from_le_bytes([data[i], data[i + 1]]);
        let version_raw = data[8];
        let firmware = ((version_raw & 0x0F) as f32
            + ((version_raw & 0xF0) >> 4) as f32 * 10.0
            + data[9] as f32 * 100.0)
            / 100.0;
        Self {
            rom_size: data[0],
            macro_space: u16_at(2),
            vid: u16_at(4),
            pid: u16_at(6),
            firmware,
            sensor: u16_at(10),
            vendor_id: u16_at(12),
            product_id: u16_at(14),
            work_mode: data[16],
            battery: data[17],
            charging: data[18],
            profile: data[19],
        }
    }
}

/// 一个已打开的键盘。
pub struct Keyboard {
    device: hidapi::HidDevice,
    report_len: usize,
    info: DeviceInfo,
    /// 实际连上的那个厂商通道，排查问题时有用。
    pub usage_page: u16,
    /// 操作系统层面看到的 PID（2.4G 时是接收器的 0xFEF9）。
    pub transport_pid: u16,
}

impl Keyboard {
    /// 找到并打开键盘，读出设备信息。
    ///
    /// 会遍历 Sonix VID 下的厂商通道，逐个握手，第一个应答的即视为键盘。
    pub fn open() -> Result<Self> {
        let api = hidapi::HidApi::new().map_err(|e| Error::Io(e.to_string()))?;

        let mut candidates: Vec<(u16, u16, std::ffi::CString)> = Vec::new();
        for info in api.device_list() {
            if info.vendor_id() != SONIX_VID || info.product_id() == 0 {
                continue;
            }
            let page = info.usage_page();
            if page & 0xFF00 != 0xFF00 {
                continue; // 只看厂商自定义通道
            }
            candidates.push((page, info.product_id(), info.path().to_owned()));
        }

        if candidates.is_empty() {
            return Err(Error::NoDevice);
        }
        // 优先实测可用的 0xFF60
        candidates.sort_by_key(|(page, _, _)| {
            CONFIG_USAGE_PAGES.iter().position(|p| p == page).unwrap_or(CONFIG_USAGE_PAGES.len())
        });

        let mut last_error = None;
        for (usage_page, transport_pid, path) in candidates {
            let device = match api.open_path(&path) {
                Ok(d) => d,
                Err(e) => {
                    last_error = Some(Error::Open(e.to_string()));
                    continue;
                }
            };
            let _ = device.set_blocking_mode(true);

            let mut kb = Self {
                device,
                report_len: protocol::DEFAULT_REPORT_LEN,
                info: DeviceInfo::parse(&[0; 20]),
                usage_page,
                transport_pid,
            };

            match kb.read_device_info() {
                Ok(info) => {
                    kb.info = info;
                    return Ok(kb);
                }
                Err(e) => last_error = Some(e),
            }
        }

        Err(last_error.unwrap_or(Error::NoDevice))
    }

    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    /// 读全部 128 个键位（每键 4 字节，共 512 字节）。
    ///
    /// NUT75 实际使用前 113 个位置，其余为全 0（未定义）。
    pub fn keymap(&self) -> Result<Vec<KeyBinding>> {
        let data = self.read_key_table(cmd::GET_KEY)?;
        Ok(data.chunks_exact(4).map(|c| KeyBinding([c[0], c[1], c[2], c[3]])).collect())
    }

    /// 重新读一次设备信息（电量会变，UI 想刷新时用）。
    pub fn refresh(&mut self) -> Result<&DeviceInfo> {
        self.info = self.read_device_info()?;
        Ok(&self.info)
    }

    /// 读 FN 层键位（结构与主键位表一致）。
    pub fn fn_keys(&self) -> Result<Vec<KeyBinding>> {
        let data = self.read_key_table(cmd::GET_FN_KEY)?;
        Ok(data.chunks_exact(4).map(|c| KeyBinding([c[0], c[1], c[2], c[3]])).collect())
    }

    /// 读单个键位。只取 4 字节，不用拉整张表。
    pub fn single_key(&self, index: usize) -> Result<KeyBinding> {
        let addr = (index * 4) as u32;
        let data = self.transfer_from(cmd::GET_KEY, addr, 4, &[])?;
        if data.len() < 4 {
            return Err(Error::Io(format!("键位 {index} 只返回 {} 字节", data.len())));
        }
        Ok(KeyBinding([data[0], data[1], data[2], data[3]]))
    }

    /// 主灯效（16 字节）。
    pub fn led_effect(&self) -> Result<LedEffect> {
        let data = self.transfer(cmd::GET_LED_EFFECT, LedEffect::LEN, &[])?;
        if data.len() < LedEffect::LEN {
            return Err(Error::Io(format!("灯效只返回 {} 字节", data.len())));
        }
        Ok(LedEffect::parse(&data))
    }

    /// 侧灯（24 字节）。
    pub fn side_light(&self) -> Result<SideLight> {
        let data = self.transfer(cmd::GET_SIDE_LIGHT, SideLight::LEN, &[])?;
        if data.len() < SideLight::LEN {
            return Err(Error::Io(format!("侧灯只返回 {} 字节", data.len())));
        }
        Ok(SideLight::parse(&data))
    }

    /// 灯箱（24 字节，结构同侧灯）。
    pub fn light_box(&self) -> Result<SideLight> {
        let data = self.transfer(cmd::GET_LIGHT_BOX, SideLight::LEN, &[])?;
        if data.len() < SideLight::LEN {
            return Err(Error::Io(format!("灯箱只返回 {} 字节", data.len())));
        }
        Ok(SideLight::parse(&data))
    }

    /// 自定义灯效：128 个 LED 的颜色。
    pub fn custom_led(&self) -> Result<Vec<LedColor>> {
        let data = self.transfer(cmd::GET_CUSTOM_LED_DATA, 512, &[])?;
        if data.len() < 512 {
            return Err(Error::Io(format!("自定义灯效只返回 {} 字节", data.len())));
        }
        Ok(LedColor::parse_all(&data))
    }

    /// 设置页（回报率、按键延时、休眠等）。
    pub fn game_mode(&self) -> Result<GameMode> {
        let data = self.transfer(cmd::GET_GAME_MODE, GameMode::LEN, &[])?;
        if data.len() < GameMode::LEN {
            return Err(Error::Io(format!("设置只返回 {} 字节", data.len())));
        }
        Ok(GameMode::parse(&data))
    }

    /// 读设备序列号。
    ///
    /// 这条命令的应答长度放在帧头的 `[2]`，不是固定值，所以单独处理。
    pub fn serial(&self) -> Result<String> {
        let data = self.transfer(cmd::GET_DEVICE_SN, 56, &[])?;
        let end = data.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        Ok(String::from_utf8_lossy(&data[..end]).into_owned())
    }

    /// 读所有非空的宏。
    ///
    /// 先拉 400 字节索引表，再对每个占用槽位读 4 字节头拿动作数，最后读动作体。
    pub fn macros(&self) -> Result<Vec<Macro>> {
        let index = self.transfer(cmd::GET_MACRO, macros::INDEX_BYTES, &[])?;
        if index.len() < macros::INDEX_BYTES {
            return Err(Error::Io(format!("宏索引表只有 {} 字节", index.len())));
        }

        let mut out = Vec::new();
        for (id, address) in macros::occupied_slots(&index) {
            let header = self.transfer_from(cmd::GET_MACRO, address, 4, &[])?;
            let count = macros::action_count(&header);
            if count == 0 {
                out.push(Macro { id, address, actions: Vec::new() });
                continue;
            }
            let body = self.transfer_from(cmd::GET_MACRO, address + 4, count * 4, &[])?;
            let actions = body.chunks_exact(4).map(Action::parse).collect();
            out.push(Macro { id, address, actions });
        }
        Ok(out)
    }

    // ---- 写入 ----
    //
    // 这些方法**会真的改键盘**。调用前先 `Keyboard::backup` 留一份底
    // （见 `examples/backup.rs`），写错了还能对回去；彻底失控才用
    // `SET_FACTORY_RESET`，那是恢复出厂而不是恢复你的配置。

    /// 写整张键位表（128 键 × 4 字节）。
    ///
    /// 少于 128 项时，剩余位置写 0（= 出厂默认）。
    pub fn set_keymap(&self, keys: &[KeyBinding]) -> Result<()> {
        self.write_table(cmd::SET_KEY, keys)
    }

    /// 写整张 FN 层键位表。
    pub fn set_fn_keys(&self, keys: &[KeyBinding]) -> Result<()> {
        self.write_table(cmd::SET_FN_KEY, keys)
    }

    /// 写主灯效。
    pub fn set_led_effect(&self, effect: &LedEffect) -> Result<()> {
        self.transfer(cmd::SET_LED_EFFECT, LedEffect::LEN, &effect.encode()).map(|_| ())
    }

    pub fn set_side_light(&self, light: &SideLight) -> Result<()> {
        self.transfer(cmd::SET_SIDE_LIGHT, SideLight::LEN, &light.encode()).map(|_| ())
    }

    pub fn set_light_box(&self, light: &SideLight) -> Result<()> {
        self.transfer(cmd::SET_LIGHT_BOX, SideLight::LEN, &light.encode()).map(|_| ())
    }

    /// 写自定义灯效（128 个 LED）。
    pub fn set_custom_led(&self, colors: &[LedColor]) -> Result<()> {
        let payload = LedColor::encode_all(colors);
        if payload.len() < 512 {
            return Err(Error::Io(format!("自定义灯效需要 128 个 LED，给了 {}", colors.len())));
        }
        self.transfer(cmd::SET_CUSTOM_LED_DATA, 512, &payload).map(|_| ())
    }

    /// 写设置（回报率 / 按键延时 / 休眠 …）。
    ///
    /// 注意 2.4G 用的字段是 `wireless_report_rate`，不是 `report_rate`。
    pub fn set_game_mode(&self, settings: &GameMode) -> Result<()> {
        self.transfer(cmd::SET_GAME_MODE, GameMode::LEN, &settings.encode()).map(|_| ())
    }

    /// 写宏：先写 400 字节索引表，再写数据区。
    ///
    /// 传入的是**完整的一份宏表**（不是增量）。空动作的槽位会被当成空槽，
    /// 数据区里不占位置——所以"删掉第 3 个宏"就是把不含它的列表再写一次。
    ///
    /// 两段的顺序和尾包标志都是官方约定：索引表不带尾包标志，数据区带。
    /// 顺序反了设备会按新索引去读还没写进去的数据，等于把宏读成一堆乱码。
    pub fn set_macros(&self, macros: &[Macro]) -> Result<()> {
        let (index, data) = macros::encode_table(macros);

        let packets = protocol::plan_chunks_opts(
            cmd::SET_MACRO,
            0,
            macros::INDEX_BYTES,
            self.report_len,
            &index,
            false,
        );
        self.send_packets(cmd::SET_MACRO, packets)?;

        if data.is_empty() {
            return Ok(());
        }
        let packets = protocol::plan_chunks(
            cmd::SET_MACRO,
            macros::DATA_OFFSET,
            data.len(),
            self.report_len,
            &data,
        );
        self.send_packets(cmd::SET_MACRO, packets)
    }

    /// 删掉某个槽位的宏（其它宏原样保留）。
    pub fn clear_macro(&self, slot: u8) -> Result<()> {
        let mut table = self.macros()?;
        table.retain(|entry| entry.id != slot);
        self.set_macros(&table)
    }

    fn write_table(&self, command: u8, keys: &[KeyBinding]) -> Result<()> {
        let mut buffer = vec![0u8; 512];
        for (index, key) in keys.iter().take(128).enumerate() {
            buffer[index * 4..index * 4 + 4].copy_from_slice(&key.raw());
        }
        self.transfer(command, 512, &buffer).map(|_| ())
    }

    /// 读一张 512 字节的键位表。
    fn read_key_table(&self, command: u8) -> Result<Vec<u8>> {
        let data = self.transfer(command, 512, &[])?;
        if data.len() < 512 {
            return Err(Error::Io(format!("键位表不完整：{} / 512 字节", data.len())));
        }
        Ok(data)
    }

    fn read_device_info(&self) -> Result<DeviceInfo> {
        let data = self.transfer(cmd::GET_DEVICE_INFO, 48, &[])?;
        if data.len() < 20 {
            return Err(Error::Io(format!("设备信息只有 {} 字节", data.len())));
        }
        Ok(DeviceInfo::parse(&data))
    }

    /// 一次完整的读传输：分片发送、逐片等应答、拼回数据。
    fn transfer(&self, command: u8, content_size: usize, payload: &[u8]) -> Result<Vec<u8>> {
        self.transfer_from(command, 0, content_size, payload)
    }

    /// 同 [`Self::transfer`]，但起始地址可指定。读宏要用（数据区从 400 开始）。
    fn transfer_from(
        &self,
        command: u8,
        addr_start: u32,
        content_size: usize,
        payload: &[u8],
    ) -> Result<Vec<u8>> {
        let packets =
            protocol::plan_chunks(command, addr_start, content_size, self.report_len, payload);
        self.exchange(command, packets)
    }

    /// 只发不收：把已经切好的分片逐条写下去。
    fn send_packets(&self, command: u8, packets: Vec<protocol::Packet>) -> Result<()> {
        self.exchange(command, packets).map(|_| ())
    }

    /// 逐片发送并等应答，把每片应答里声明的字节拼回一条数据。
    fn exchange(&self, command: u8, packets: Vec<protocol::Packet>) -> Result<Vec<u8>> {
        let mut out = Vec::new();

        for packet in packets {
            let frame_bytes = packet.encode(self.report_len);
            // hidapi 的 write 要求首字节是 report id
            let mut frame = Vec::with_capacity(frame_bytes.len() + 1);
            frame.push(0);
            frame.extend_from_slice(&frame_bytes);

            let mut attempt = 0;
            let mut response = None;
            while attempt <= MAX_RETRIES && response.is_none() {
                self.device.write(&frame).map_err(|e| Error::Io(e.to_string()))?;
                response = self.read_response(command);
                attempt += 1;
            }

            let response = response.ok_or(Error::Timeout { cmd: command })?;
            // 只取本片声明的长度，避免把帧尾填充带进结果
            out.extend_from_slice(&response.data[..response.data.len().min(packet.size)]);
        }

        Ok(out)
    }

    /// 等一条 cmd 匹配的应答，期间丢弃其它报文。
    fn read_response(&self, expected_cmd: u8) -> Option<protocol::Response> {
        let mut buffer = [0u8; 256];
        for _ in 0..8 {
            let read = self.device.read_timeout(&mut buffer, RESPONSE_TIMEOUT_MS).ok()?;
            if read == 0 {
                return None;
            }
            if let Some(response) = protocol::Response::parse(&buffer[..read]) {
                if response.cmd == expected_cmd {
                    return Some(response);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机 `GET_DEVICE_INFO` 返回的 48 字节前 20 字节。
    const REAL_INFO: [u8; 20] = [
        0x40, 0x00, // ROM 64
        0x00, 0x0C, // 宏空间 3072
        0x45, 0x0C, // vid 0x0C45
        0x0C, 0x88, // pid 0x880C
        0x45, 0x01, // 版本：1.45
        0x00, 0x00, // 传感器
        0x11, 0x00, // 厂商 ID 0x0011
        0x1C, 0x00, // 产品 ID 0x001C
        0x02, // 工作模式 2 = 2.4G
        0x3D, // 电量 61
        0x00, // 未充电
        0x00, // 配置档 0
    ];

    #[test]
    fn parses_device_info_from_real_hardware() {
        let info = DeviceInfo::parse(&REAL_INFO);
        assert_eq!(info.rom_size, 64);
        assert_eq!(info.macro_space, 3072);
        assert_eq!(info.vid, 0x0C45);
        assert_eq!(info.pid, 0x880C);
        assert_eq!(info.firmware, 1.45);
        assert_eq!(info.product_id, NUT75_PRODUCT_ID);
        assert!(info.is_nut75());
        assert_eq!(info.connection(), "2.4G");
        assert_eq!(info.battery, 61);
    }
}
