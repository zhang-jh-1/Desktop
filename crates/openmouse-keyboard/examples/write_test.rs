//! 写入链路验证：三条独立检查，逐条跑、每条都靠**读回比对**判定成败。
//!
//! ```text
//! cargo run --release --example write_test -- settings   # 设置原值写回（不动任何值）
//! cargo run --release --example write_test -- keymap     # 键位表原值写回（不动任何值）
//! cargo run --release --example write_test -- change     # 真改一个键 → 读回 → 还原 → 读回
//! ```
//!
//! 设计原则：**不信写入的返回值**。超时也可能已经写进去了，只有读回一致才算
//! 成功。所以每一步都先读一遍、写、再读一遍，逐字节比。
//!
//! 改键用的是键位 105（PgUp），临时改成 Home，验证完立刻写回原值。就算中途
//! 崩了，影响也只是 PgUp 变成 Home，改回来即可。

use openmouse_keyboard::{KeyBinding, Keyboard};

/// 用来做写入测试的物理键位。
const TEST_INDEX: usize = 105;
/// 它的出厂值（实测读回）。
const TEST_ORIGINAL: [u8; 4] = [2, 0, 75, 0];
/// 临时改成的值：Home（usage 0x4A）。
const TEST_TEMP: [u8; 4] = [2, 0, 74, 0];

fn main() {
    let phase = std::env::args().nth(1).unwrap_or_else(|| "settings".to_string());
    match run(&phase) {
        Ok(()) => println!("\n[{phase}] 通过"),
        Err(e) => {
            eprintln!("\n[{phase}] 失败：{e}");
            std::process::exit(1);
        }
    }
}

fn run(phase: &str) -> openmouse_keyboard::Result<()> {
    let kb = Keyboard::open()?;
    println!(
        "设备 {} / 固件 {} / 电量 {}% / 连接 {}",
        if kb.info().is_nut75() { "NUT75" } else { "未知型号" },
        kb.info().firmware,
        kb.info().battery,
        kb.info().connection()
    );

    match phase {
        "settings" => check_settings(&kb),
        "settings-change" => check_settings_change(&kb),
        "keymap" => check_keymap(&kb),
        "change" => check_change(&kb),
        other => {
            eprintln!("未知阶段 `{other}`，可用：settings / settings-change / keymap / change");
            Ok(())
        }
    }
}

/// 把设置原样写回去。不改任何值，只验证写入通道通不通。
fn check_settings(kb: &Keyboard) -> openmouse_keyboard::Result<()> {
    println!("\n== 设置原值写回 ==");
    let before = kb.game_mode()?;
    println!("读回 56 字节: {}", hex(&before.raw()));

    kb.set_game_mode(&before)?;
    println!("已发送 SET_GAME_MODE");

    let after = kb.game_mode()?;
    println!("再读 56 字节: {}", hex(&after.raw()));

    if before.raw() == after.raw() {
        println!("✅ 56 字节完全一致");
        Ok(())
    } else {
        let diff = first_diff(&before.raw(), &after.raw());
        Err(openmouse_keyboard::Error::Io(format!(
            "写回后设置变了，首个差异在第 {} 字节：{:02X} -> {:02X}",
            diff.0, diff.1, diff.2
        )))
    }
}

/// 真改一个设置字段再还原。
///
/// 挑的是**按键延时**（`[4]`，范围 1–5），不是回报率。回报率在 2.4G 下改动
/// 可能让链路重连，万一接不回来键盘就掉线了，不值得为验证一个已知的映射冒
/// 这个险。按键延时只影响去抖节奏，写错也无感。
fn check_settings_change(kb: &Keyboard) -> openmouse_keyboard::Result<()> {
    println!("\n== 真改一个设置字段再还原 ==");

    let before = kb.game_mode()?;
    let original = before.key_delay;
    println!(
        "按键延时当前 = {original}（回报率码值 {} = {:?}）",
        before.report_rate,
        before.report_rate_hz_for_nut75()
    );
    let target = if original == 4 { 3 } else { 4 };

    // --- 改 ---
    kb.set_game_mode(&before.with(|s| s.key_delay = target))?;
    let after = kb.game_mode()?;
    println!("写入 {target} 后读回 = {}", after.key_delay);
    if after.key_delay != target {
        let _ = kb.set_game_mode(&before);
        return Err(openmouse_keyboard::Error::Io(format!(
            "按键延时没写进去，读回还是 {}",
            after.key_delay
        )));
    }
    println!("✅ 写入生效");

    // 确认只动了这一个字节
    let diff: Vec<usize> = before
        .raw()
        .iter()
        .zip(after.raw().iter())
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    if diff != vec![4] {
        let _ = kb.set_game_mode(&before);
        return Err(openmouse_keyboard::Error::Io(format!(
            "预期只改第 4 字节，实际变了 {diff:?}，已还原"
        )));
    }
    println!("✅ 56 字节里只有 [4] 变化，其余保留位没被清零");

    // --- 还原 ---
    println!("\n还原中…");
    kb.set_game_mode(&before)?;
    let restored = kb.game_mode()?;
    if restored.raw() == before.raw() {
        println!("✅ 已完整还原，56 字节与原始值一致");
        Ok(())
    } else {
        let (i, a, b) = first_diff(&before.raw(), &restored.raw());
        Err(openmouse_keyboard::Error::Io(format!(
            "还原不完整，第 {i} 字节 {a:02X} -> {b:02X}"
        )))
    }
}

/// 把整张键位表原样写回去。
fn check_keymap(kb: &Keyboard) -> openmouse_keyboard::Result<()> {
    println!("\n== 键位表原值写回 ==");
    let before = kb.keymap()?;
    println!("读回 {} 个键位，非空 {}", before.len(), before.iter().filter(|k| !k.is_default()).count());

    kb.set_keymap(&before)?;
    println!("已发送 SET_KEY（512 字节）");

    let after = kb.keymap()?;
    println!("再读 {} 个键位", after.len());

    let mut bad = Vec::new();
    for (i, (a, b)) in before.iter().zip(after.iter()).enumerate() {
        if a.raw() != b.raw() {
            bad.push((i, a.raw(), b.raw()));
        }
    }
    if bad.is_empty() {
        println!("✅ {} 个键位全部一致", before.len());
        Ok(())
    } else {
        for (i, a, b) in bad.iter().take(5) {
            println!("  键位 {i}: {:?} -> {:?}", a, b);
        }
        Err(openmouse_keyboard::Error::Io(format!("{} 个键位写回后不一致", bad.len())))
    }
}

/// 真改一个键，确认能写进去；然后立刻还原，确认能改回来。
fn check_change(kb: &Keyboard) -> openmouse_keyboard::Result<()> {
    println!("\n== 真改一个键再还原 ==");

    let before = kb.keymap()?;
    let current = before[TEST_INDEX].raw();
    println!("键位 {TEST_INDEX} 当前值: {:?}  ({})", current, before[TEST_INDEX].describe());
    if current != TEST_ORIGINAL {
        return Err(openmouse_keyboard::Error::Io(format!(
            "键位 {TEST_INDEX} 的当前值不是预期的 {TEST_ORIGINAL:?}，为免误改，中止"
        )));
    }

    // --- 改成 Home ---
    let mut edited = before.clone();
    edited[TEST_INDEX] = KeyBinding(TEST_TEMP);
    kb.set_keymap(&edited)?;
    let after_write = kb.keymap()?[TEST_INDEX].raw();
    println!("写入 {:?} 后读回: {:?}", TEST_TEMP, after_write);
    if after_write != TEST_TEMP {
        // 立刻尝试还原再报错
        let _ = kb.set_keymap(&before);
        return Err(openmouse_keyboard::Error::Io("写入没生效（读回仍是旧值）".into()));
    }
    println!("✅ 写入生效");

    // 顺带确认只动了这一个键
    let round = kb.keymap()?;
    let touched: Vec<usize> = round
        .iter()
        .zip(before.iter())
        .enumerate()
        .filter(|(_, (a, b))| a.raw() != b.raw())
        .map(|(i, _)| i)
        .collect();
    if touched != vec![TEST_INDEX] {
        let _ = kb.set_keymap(&before);
        return Err(openmouse_keyboard::Error::Io(format!(
            "预期只改键位 {TEST_INDEX}，实际被改了 {touched:?}，已还原"
        )));
    }
    println!("✅ 只有键位 {TEST_INDEX} 变化，其它 {} 个键没被牵连", round.len() - 1);

    // --- 还原 ---
    println!("\n还原中…");
    kb.set_keymap(&before)?;
    let restored = kb.keymap()?;
    let mut bad = Vec::new();
    for (i, (a, b)) in before.iter().zip(restored.iter()).enumerate() {
        if a.raw() != b.raw() {
            bad.push(i);
        }
    }
    if bad.is_empty() {
        println!("✅ 已完整还原，512 字节与原始值一致");
        Ok(())
    } else {
        Err(openmouse_keyboard::Error::Io(format!("还原不完整，键位 {bad:?} 与原始值不同")))
    }
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

fn first_diff(a: &[u8], b: &[u8]) -> (usize, u8, u8) {
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        if x != y {
            return (i, *x, *y);
        }
    }
    (a.len(), 0, 0)
}
