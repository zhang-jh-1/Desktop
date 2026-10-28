//! 宏写入验证：写一个宏 → 读回比对 → 删掉 → 再读回确认清空。
//!
//! ```bash
//! cd crates/openmouse-keyboard && cargo run --release --example macro_test
//! ```
//!
//! 宏是**两段式写入**（先 400 字节索引表、再数据区），顺序或尾包标志错了
//! 设备会按新索引去读还没写进去的数据。所以这里每写一步都读回比对，最后
//! 一定把键盘恢复成"没有任何宏"的原始状态。
//!
//! 写的测试宏是 Ctrl+C：四条动作，键码用 USB HID 的 A=0x04 那套编号。

use openmouse_keyboard::{Action, Keyboard, Macro};

/// 测试用的槽位。0 号槽，写完就删。
const SLOT: u8 = 0;

fn main() {
    match run() {
        Ok(()) => println!("\n通过"),
        Err(e) => {
            eprintln!("\n失败：{e}");
            std::process::exit(1);
        }
    }
}

fn run() -> openmouse_keyboard::Result<()> {
    let kb = Keyboard::open()?;
    println!(
        "设备 NUT75={} / 固件 {} / 电量 {}% / 连接 {}",
        kb.info().is_nut75(),
        kb.info().firmware,
        kb.info().battery,
        kb.info().connection()
    );

    // --- 0. 原始状态 ---
    let before = kb.macros()?;
    println!("\n== 原始状态 ==\n已保存的宏：{} 个", before.len());
    for entry in &before {
        println!("  槽 {} / {} 个动作 / 偏移 {}", entry.id, entry.actions.len(), entry.address);
    }
    if !before.is_empty() {
        return Err(openmouse_keyboard::Error::Io(
            "键盘上已经有宏，为免覆盖用户数据，先用界面或备份确认后再跑本测试".into(),
        ));
    }

    // --- 1. 写入 Ctrl+C ---
    let test_macro = Macro {
        id: SLOT,
        address: 0,
        actions: vec![
            Action::keyboard(0xE0, true, 10),  // Ctrl 按下
            Action::keyboard(0x06, true, 30),  // C 按下
            Action::keyboard(0x06, false, 40), // C 抬起
            Action::keyboard(0xE0, false, 10), // Ctrl 抬起
        ],
    };
    println!("\n== 写入宏 ==");
    println!("槽 {SLOT}：4 个动作，Ctrl+C");
    kb.set_macros(&[test_macro.clone()])?;
    println!("已发送 SET_MACRO（索引表 400 B + 数据区 20 B）");

    // --- 2. 读回比对 ---
    let after = kb.macros()?;
    println!("\n== 读回 ==");
    println!("已保存的宏：{} 个", after.len());
    let Some(read_back) = after.iter().find(|m| m.id == SLOT) else {
        return Err(openmouse_keyboard::Error::Io("写进去的宏读不回来".into()));
    };
    println!("  槽 {} / {} 个动作 / 偏移 {}", read_back.id, read_back.actions.len(), read_back.address);
    for (i, action) in read_back.actions.iter().enumerate() {
        println!(
            "    [{i}] {} 键码 0x{:02X} 延时 {}ms  flags 0x{:02X}",
            if action.is_press() { "按下" } else { "抬起" },
            action.key_code,
            action.delay,
            action.flags
        );
    }
    if read_back.actions != test_macro.actions {
        let _ = kb.set_macros(&[]);
        return Err(openmouse_keyboard::Error::Io(format!(
            "动作不一致：写入 {:?}，读回 {:?}（已尝试清空）",
            test_macro.actions, read_back.actions
        )));
    }
    if read_back.address != 400 {
        return Err(openmouse_keyboard::Error::Io(format!(
            "数据区偏移应为 400，实际 {}",
            read_back.address
        )));
    }
    println!("✅ 4 个动作逐条一致，数据区偏移 400");

    // --- 3. 删掉 ---
    println!("\n== 清空 ==");
    kb.set_macros(&[])?;
    println!("已写回空表");
    let cleared = kb.macros()?;
    println!("再读：{} 个宏", cleared.len());
    if !cleared.is_empty() {
        return Err(openmouse_keyboard::Error::Io(format!(
            "清空后还剩 {} 个宏",
            cleared.len()
        )));
    }
    println!("✅ 已恢复成没有任何宏的状态");
    Ok(())
}
