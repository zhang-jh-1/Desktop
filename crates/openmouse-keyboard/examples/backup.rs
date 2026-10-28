//! 配置备份：把能读的东西全部存成 JSON。
//!
//! 写入任何配置之前都应该先跑一次备份——`SET_FACTORY_RESET` 能救场，但那是
//! 恢复出厂，不是恢复**你的**配置。

use openmouse_keyboard::{
    GameMode, KeyBinding, Keyboard, LedColor, LedEffect, Macro, SideLight,
};
use serde::Serialize;

#[derive(Serialize)]
struct DeviceSnapshot {
    info: openmouse_keyboard::DeviceInfo,
    usage_page: u16,
    transport_pid: u16,
    serial: Option<String>,
    keys: Vec<KeyBinding>,
    fn_keys: Vec<KeyBinding>,
    led_effect: Option<LedEffect>,
    side_light: Option<SideLight>,
    light_box: Option<SideLight>,
    custom_led: Vec<LedColor>,
    settings: Option<GameMode>,
    macros: Vec<Macro>,
}

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| "nut75-backup.json".to_string());

    let kb = match Keyboard::open() {
        Ok(kb) => kb,
        Err(e) => {
            println!("打不开键盘：{e}");
            return;
        }
    };

    // 每一项单独兜底：某一项读不到不该让整份备份失败
    let snapshot = DeviceSnapshot {
        info: kb.info().clone(),
        usage_page: kb.usage_page,
        transport_pid: kb.transport_pid,
        serial: kb.serial().ok().filter(|s| !s.is_empty()),
        keys: kb.keymap().unwrap_or_default(),
        fn_keys: kb.fn_keys().unwrap_or_default(),
        led_effect: kb.led_effect().ok(),
        side_light: kb.side_light().ok(),
        light_box: kb.light_box().ok(),
        custom_led: kb.custom_led().unwrap_or_default(),
        settings: kb.game_mode().ok(),
        macros: kb.macros().unwrap_or_default(),
    };

    let json = match serde_json::to_string_pretty(&snapshot) {
        Ok(j) => j,
        Err(e) => {
            println!("序列化失败：{e}");
            return;
        }
    };

    if let Err(e) = std::fs::write(&path, &json) {
        println!("写文件失败：{e}");
        return;
    }

    println!("已备份到 {path}（{} 字节）", json.len());
    println!(
        "  键位 {} 个 / FN 层 {} 个 / 宏 {} 个",
        snapshot.keys.len(),
        snapshot.fn_keys.len(),
        snapshot.macros.len()
    );
}
