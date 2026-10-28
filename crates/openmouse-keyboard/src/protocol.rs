//! NUT75（Sonix 方案）的 HID 报文层。
//!
//! 这个模块只做字节：组帧、分片、解析应答。不碰设备、不碰 UI，所以可以单测。
//!
//! 帧格式（来自官方网页驱动的前端实现，已在真机验证）：
//!
//! ```text
//! 请求   [0]=0xAA [1]=cmd [2]=len [3..4]=addr(LE) [5..7]=子头(可选)
//!        [6]=最后一片标志（无子头时） [8..]=data
//! 应答   [0]=0x55 [1]=cmd [2]=len [3..4]=addr(LE) [5..7]=保留 [8..]=data
//! ```
//!
//! 分片大小 = `report_len - 8`，默认报告长 32 字节即每片 24 字节。

/// 请求帧魔数。
pub const MAGIC_REQUEST: u8 = 0xAA;
/// 应答帧魔数。
pub const MAGIC_RESPONSE: u8 = 0x55;
/// 帧头长度：魔数 + 命令 + 长度 + 2 字节地址 + 3 字节子头。
pub const HEADER_LEN: usize = 8;
/// 默认报文长度（HID 描述符里的 reportCount）。
pub const DEFAULT_REPORT_LEN: usize = 32;

/// 命令码。只列出这个项目用到的部分——全部 66 条见 `PROTOCOL.md`。
pub mod cmd {
    pub const COMMUNICATION_START: u8 = 0x01;
    pub const COMMUNICATION_END: u8 = 0x02;
    pub const SET_FACTORY_RESET: u8 = 0x0F;
    pub const GET_DEVICE_INFO: u8 = 0x10;
    pub const GET_GAME_MODE: u8 = 0x11;
    pub const GET_KEY: u8 = 0x12;
    pub const GET_CUSTOM_LED_DATA: u8 = 0x14;
    pub const GET_LIGHT_BOX: u8 = 0x1B;
    pub const SET_KEY: u8 = 0x22;
    pub const GET_LED_EFFECT: u8 = 0x13;
    pub const SET_LED_EFFECT: u8 = 0x23;
    pub const GET_MACRO: u8 = 0x15;
    pub const SET_MACRO: u8 = 0x25;
    pub const GET_FN_KEY: u8 = 0x16;
    pub const SET_FN_KEY: u8 = 0x26;
    pub const GET_SIDE_LIGHT: u8 = 0x1D;
    pub const SET_SIDE_LIGHT: u8 = 0x2D;
    pub const SET_CUSTOM_LED_DATA: u8 = 0x24;
    pub const SET_LIGHT_BOX: u8 = 0x2B;
    pub const SET_GAME_MODE: u8 = 0x21;
    /// 出厂键位矩阵，用于「恢复默认」。
    pub const GET_DEFAULT_KEY_MATRIX: u8 = 0x1F;
    /// 出厂 FN 层矩阵。
    pub const GET_DEFAULT_FN_KEY_MATRIX: u8 = 0x1C;
    /// 实时读取灯效数据（音乐律动用它推流）。
    pub const GET_LED_DATA: u8 = 0x32;
    pub const GET_ALL_LIGHTS_RGB: u8 = 0x33;
    pub const SET_MUSIC_DATA: u8 = 0x35;
    pub const CLEAR_LED_DATA: u8 = 0x36;
    /// 2.4G 专用的大块音乐数据（报告长 64 字节时）。
    pub const SET_MUSIC_DATA_24G_64_BYTE: u8 = 0x3C;
    /// 大块灯效数据（GIF / 逐帧动画），自带 8 字节子头。
    pub const SET_LED_DATA: u8 = 0x42;
    /// OTA 命令组里的序列号读取，NUT75 上只读不写。
    pub const GET_DEVICE_SN: u8 = 0x84;
}

/// 一个请求分片。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub cmd: u8,
    pub size: usize,
    pub addr: u16,
    pub is_last: bool,
    pub data: Vec<u8>,
}

impl Packet {
    /// `size` 是**声明**给设备的长度，和 `data` 的实际长度分开。
    /// 读命令的载荷为空，但每片仍要声明自己的长度，设备据此决定回多少字节。
    pub fn new(cmd: u8, addr: u16, size: usize, data: &[u8], is_last: bool) -> Self {
        Self { cmd, size, addr, is_last, data: data[..data.len().min(size)].to_vec() }
    }

    /// 序列化成 `report_len` 字节的 HID 输出报文（不含 report id）。
    pub fn encode(&self, report_len: usize) -> Vec<u8> {
        let mut out = vec![0u8; report_len];
        let capacity = report_len.saturating_sub(HEADER_LEN);
        out[0] = MAGIC_REQUEST;
        out[1] = self.cmd;
        out[2] = (self.size & 0xFF) as u8;
        out[3] = (self.addr & 0xFF) as u8;
        out[4] = (self.addr >> 8) as u8;
        // 第 5..7 字节是可选子头；不用子头时第 6 字节承载“最后一片”标志。
        out[6] = u8::from(self.is_last);
        let n = self.data.len().min(capacity);
        out[HEADER_LEN..HEADER_LEN + n].copy_from_slice(&self.data[..n]);
        out
    }
}

/// 一条应答。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Response {
    pub cmd: u8,
    pub addr: u16,
    /// 帧头之后的载荷（未按请求长度截断）。
    pub data: Vec<u8>,
}

impl Response {
    /// 从一条 HID 输入报文里解析。报文可能带 report id 前缀，故按魔数定位。
    pub fn parse(report: &[u8]) -> Option<Self> {
        let start = report.iter().position(|&b| b == MAGIC_RESPONSE)?;
        let body = &report[start..];
        if body.len() < HEADER_LEN {
            return None;
        }
        Some(Self {
            cmd: body[1],
            addr: u16::from_le_bytes([body[3], body[4]]),
            data: body[HEADER_LEN..].to_vec(),
        })
    }
}

/// 把一次传输切成若干个分片，地址从 `addr_start` 开始。
///
/// 读宏这类命令要从数据区中间开始读（`addr_start = 400`），所以起始地址
/// 必须能指定，不能永远从 0 算。
pub fn plan_chunks(
    cmd: u8,
    addr_start: u32,
    content_size: usize,
    report_len: usize,
    payload: &[u8],
) -> Vec<Packet> {
    plan_chunks_opts(cmd, addr_start, content_size, report_len, payload, true)
}

/// 同 [`plan_chunks`]，但可以关掉"最后一片"标志。
///
/// 写宏是两段式的：先写 400 字节索引表，再写数据区。官方驱动写索引表时
/// `isNeedLastPacketFlag = false`——设备要等数据区也写完才落盘，索引表
/// 单独带上尾包标志会让它在数据区还没到位时就先算一次校验。
pub fn plan_chunks_opts(
    cmd: u8,
    addr_start: u32,
    content_size: usize,
    report_len: usize,
    payload: &[u8],
    mark_last: bool,
) -> Vec<Packet> {
    let chunk = report_len.saturating_sub(HEADER_LEN).max(1);
    let count = content_size.div_ceil(chunk);
    (0..count)
        .map(|i| {
            let offset = i * chunk;
            let size = (content_size - offset).min(chunk);
            let slice = payload.get(offset..offset + size).unwrap_or(&[]);
            let is_last = mark_last && i + 1 == count;
            Packet::new(cmd, (addr_start as usize + offset) as u16, size, slice, is_last)
        })
        .collect()
}

/// CRC16-CCITT（多项式 0x1021，初值 0xFFFF）。
///
/// 官方驱动里有这个函数，但本项目用到的“读配置”命令并不校验它；写命令
/// 是否需要尚未验证，所以先留着，等写入功能实测后再决定要不要用。
pub fn crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 { (crc << 1) ^ 0x1021 } else { crc << 1 };
        }
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_header_fields_little_endian() {
        let p = Packet::new(cmd::GET_DEVICE_INFO, 0x0123, 24, &[], true);
        let bytes = p.encode(32);
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], MAGIC_REQUEST);
        assert_eq!(bytes[1], cmd::GET_DEVICE_INFO);
        assert_eq!(bytes[2], 24); // 声明的分片长度
        assert_eq!(bytes[3], 0x23); // 低字节
        assert_eq!(bytes[4], 0x01); // 高字节
        assert_eq!(bytes[6], 1); // 最后一片
    }

    #[test]
    fn encodes_last_flag_only_on_tail_chunk() {
        let packets = plan_chunks(cmd::GET_KEY, 0, 512, 32, &[]);
        assert_eq!(packets.len(), 512_usize.div_ceil(24));
        assert!(packets.first().unwrap().size == 24);
        assert_eq!(packets.last().unwrap().is_last, true);
        assert!(packets[..packets.len() - 1].iter().all(|p| !p.is_last));
        // 地址按片递增，最后一片从 504 开始且只有 8 字节
        assert_eq!(packets.last().unwrap().addr, 504);
        assert_eq!(packets.last().unwrap().size, 8);
    }

    #[test]
    fn chunk_addresses_start_where_asked() {
        let packets = plan_chunks(cmd::GET_MACRO, 700, 48, 32, &[]);
        assert_eq!(packets[0].addr, 700);
        assert_eq!(packets[1].addr, 724);
        assert_eq!(packets.len(), 2);
    }

    #[test]
    fn data_lands_after_the_header() {
        let data = vec![0xAB; 24];
        let bytes = Packet::new(cmd::SET_KEY, 0, 24, &data, true).encode(32);
        assert_eq!(&bytes[HEADER_LEN..HEADER_LEN + 24], &data[..]);
    }

    #[test]
    fn parses_response_with_and_without_report_id() {
        let mut body = vec![MAGIC_RESPONSE, 0x10, 24, 0x18, 0x00, 0, 0, 0];
        body.extend_from_slice(&[1, 2, 3]);
        let with_id = Response::parse(&[&[0u8][..], &body].concat()).unwrap();
        let without_id = Response::parse(&body).unwrap();
        assert_eq!(with_id.cmd, 0x10);
        assert_eq!(with_id.addr, 24);
        assert_eq!(with_id.data, vec![1, 2, 3]);
        assert_eq!(with_id, without_id);
    }

    #[test]
    fn ignores_reports_that_are_not_responses() {
        assert!(Response::parse(&[0x00, 0x01, 0x02]).is_none());
        assert!(Response::parse(&[MAGIC_RESPONSE, 0x10]).is_none());
    }

    #[test]
    fn crc16_matches_ccitt_reference() {
        // CRC-16/CCITT-FALSE("123456789") = 0x29B1
        assert_eq!(crc16(b"123456789"), 0x29B1);
    }
}
