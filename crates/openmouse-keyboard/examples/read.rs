//! 读一遍真实键盘的全部配置：设备信息、键位、FN 层、灯效、设置、宏。
//!
//! ```bash
//! cd crates/openmouse-keyboard && cargo run --release --example read
//! ```
//!
//! 全程只读，不发任何写命令。

use openmouse_keyboard::Keyboard;

fn main() {
    let kb = match Keyboard::open() {
        Ok(kb) => kb,
        Err(e) => {
            println!("打不开键盘：{e}");
            return;
        }
    };

    let info = kb.info();
    println!("== 设备 ==");
    println!("  型号        : NUT75（{}）", if info.is_nut75() { "已识别" } else { "未识别" });
    println!("  连接        : {}", info.connection());
    println!("  固件        : v{:.2}", info.firmware);
    println!("  电量        : {}%{}", info.battery, if info.charging == 0 { "" } else { "（充电中）" });
    println!("  宏空间      : {} 字节", info.macro_space);
    println!("  配置通道    : usage_page 0x{:04X}（系统 PID 0x{:04X}）", kb.usage_page, kb.transport_pid);
    match kb.serial() {
        Ok(sn) if !sn.is_empty() => println!("  序列号      : {sn}"),
        Ok(_) => println!("  序列号      : （空）"),
        Err(e) => println!("  序列号      : 读取失败 {e}"),
    }

    match kb.keymap() {
        Ok(keys) => {
            let changed = keys.iter().filter(|k| !k.is_default()).count();
            println!("\n== 键位表 ==\n  {} 个位置，{} 个有数据", keys.len(), changed);
            for (i, k) in keys.iter().enumerate().filter(|(_, k)| !k.is_default()).take(6) {
                println!("    键位 {i:3}: {}", k.describe());
            }
        }
        Err(e) => println!("\n键位表读取失败：{e}"),
    }

    match kb.single_key(0) {
        Ok(k) => println!("  单键读取 键位0: {}", k.describe()),
        Err(e) => println!("  单键读取失败：{e}"),
    }

    match kb.fn_keys() {
        Ok(keys) => {
            let changed = keys.iter().filter(|k| !k.is_default()).count();
            println!("\n== FN 层 ==\n  {} 个位置，{} 个有数据", keys.len(), changed);
            for (i, k) in keys.iter().enumerate().filter(|(_, k)| !k.is_default()).take(6) {
                println!("    键位 {i:3}: {}", k.describe());
            }
        }
        Err(e) => println!("\nFN 层读取失败：{e}"),
    }

    match kb.led_effect() {
        Ok(l) => println!(
            "\n== 主灯效 ==\n  模式 {} / 颜色 ({}, {}, {}) / 亮度 {} / 速度 {} / colorMode {}\n  副色 ({}, {}, {}) / direction {} / effectType {}\n  校验 {} {}",
            l.mode, l.red, l.green, l.blue, l.brightness, l.speed, l.color_mode,
            l.secondary_red, l.secondary_green, l.secondary_blue, l.direction, l.effect_mode_type,
            l.check_code[0], l.check_code[1]
        ),
        Err(e) => println!("\n主灯效读取失败：{e}"),
    }

    for (label, result) in [("侧灯", kb.side_light()), ("灯箱", kb.light_box())] {
        match result {
            Ok(l) => println!(
                "\n== {label} ==\n  模式 {} / 颜色 ({}, {}, {}) / 亮度 {} / 速度 {}",
                l.mode, l.red, l.green, l.blue, l.brightness, l.speed
            ),
            Err(e) => println!("\n{label}读取失败：{e}"),
        }
    }

    match kb.custom_led() {
        Ok(leds) => {
            let lit = leds.iter().filter(|l| l.red + l.green + l.blue > 0).count();
            println!("\n== 自定义灯效 ==\n  {} 个 LED，其中 {} 个有颜色", leds.len(), lit);
            for l in leds.iter().filter(|l| l.red + l.green + l.blue > 0).take(5) {
                println!("    LED {:3}: ({}, {}, {})", l.led_id, l.red, l.green, l.blue);
            }
        }
        Err(e) => println!("\n自定义灯效读取失败：{e}"),
    }

    match kb.game_mode() {
        Ok(g) => println!(
            "\n== 设置 ==\n  有线回报率 {} / 2.4G 回报率 {}\n  按键延时 {} / 休眠档 {} / Fn 层 {} / NKRO {}\n  游戏模式 {} / 系统模式 {} / 电源模式 {}",
            g.report_rate, g.wireless_report_rate, g.key_delay, g.sleep_time,
            g.fn_switch, g.nkro_switch, g.game_mode, g.system_mode, g.power_mode
        ),
        Err(e) => println!("\n设置读取失败：{e}"),
    }

    match kb.macros() {
        Ok(list) => {
            println!("\n== 宏 ==");
            if list.is_empty() {
                println!("  没有已保存的宏");
            } else {
                println!("  {} 个宏", list.len());
                for m in list.iter().take(5) {
                    println!("    宏 {:2} @ 偏移 {}: {} 个动作", m.id, m.address, m.actions.len());
                    for (i, a) in m.actions.iter().take(3).enumerate() {
                        println!(
                            "      动作 {i}: 延时 {}ms 键码 0x{:02X} {}",
                            a.delay,
                            a.key_code,
                            if a.is_press() { "按下" } else { "抬起" }
                        );
                    }
                }
            }
        }
        Err(e) => println!("\n宏读取失败：{e}"),
    }

    println!("\n读取完毕（未发送任何写命令）。");
}
