# 包结构

鼠标和键盘是**两个互不引用的领域包**，只在最底层的 HID 传输上共用。UI 是第三个东西，
它同时依赖两者，但两者之间没有任何 `use` 关系。

```
openmouse-desktop/
├─ crates/
│  ├─ openmouse-hid/        ← 共用底层
│  │    原生 HID 传输（枚举、open、读写、reader 线程）
│  │    proc::quiet()      启动子进程时隐藏控制台窗口
│  │
│  ├─ openmouse-mouse/      ← 鼠标（703 行，原 gpui/src/hid.rs）
│  │    HidService         QuickJS 宿主，在内嵌运行时里跑 @openmouse/protocol
│  │    Candidate           设备候选（VID/PID + 厂商通道）
│  │    supported_devices  --list-supported 用的设备清单导出
│  │
│  ├─ openmouse-keyboard/   ← 键盘（维咖 NUT75 / Sonix 方案）
│  │    Keyboard::open()   发现设备、握手、读设备信息
│  │    Keyboard::keymap() 读 128 个键位（每键 4 字节）
│  │    protocol          报文组帧、分片、应答解析、CRC16（纯函数，可单测）
│  │    keymap            键位编码/解码（Page 枚举 + 4 字节布局）
│  │
│  └─ openmouse-presence/   Discord Rich Presence
│
└─ gpui/                    ← UI 壳，只依赖上面几个包
      src/app.rs            界面
      js/dist/bundle.js     鼠标协议的前端 bundle（被 openmouse-mouse 用 include_str! 嵌入）
```

## 依赖方向

```
              gpui（UI）
             ╱          ╲
   openmouse-mouse    openmouse-keyboard
             ╲          ╱
            openmouse-hid
```

- `openmouse-mouse` **不依赖** `openmouse-keyboard`，反之亦然。
- 两者都能被单独编译、单独测试：`cd crates/openmouse-keyboard && cargo test`。
- 共用的是 `openmouse-hid` 这层系统调用封装；键盘包目前直接用 `hidapi`，
  等鼠标侧的 `hid_send_report` 抽象稳定下来再考虑统一（见下方“待办”）。

## 为什么这么切

`gpui/src/hid.rs` 原本就是鼠标领域代码——QuickJS 宿主、雷蛇/罗技的设备发现、贴图、板载配置，
只依赖 `crate::proc` 一处，边界天然干净。把它移出 app 之后：

- app 的 `lib.rs` 从 `pub mod hid;` 变成 `pub use openmouse_mouse as hid;`，
  `app.rs` 里 18 处 `crate::hid::…` 一行没改。
- 键盘可以按自己的节奏长（灯效、宏、高级键），不必挤进鼠标的 QuickJS 那套抽象。

## 验证

| 项 | 结果 |
|---|---|
| `openmouse-keyboard` 单元测试 | 11 passed（报文、分片、CRC、键位编解码、真机数据解析） |
| 键盘读真机 | `cargo run --example read` → NUT75 已识别 / 2.4G / v1.45 / 61% / 113 个键位 |
| 鼠标功能回归 | 应用启动后总览仍列出 Razer Viper V2 Pro，69% · 无线 |
| 整体编译 | `cargo build --release` EXIT=0，除中文路径导致的图标告警外无警告 |

## 键盘 UI（`gpui/src/app/keyboard.rs`）

键盘的界面代码是 `app` 的**子模块**，不是独立 crate：这样它能直接读写 `AppView`
的私有字段，又不用把整块 UI 塞回已经 4400 行的 `app.rs`。

| 位置 | 内容 |
|---|---|
| 总览页 | 键盘作为一张卡片参与同一个网格（`cols` 把键盘算进卡片数），标题「NUT75」，副标题「2.4G · 61% · 固件 v1.45」，卡片里画 6 行键帽剪影代替产品图 |
| 详情页 | 五个标签：改键 / 灯效 / 宏 / 高级键 / 设置 |
| 改键 | 79 键的键帽网格（`layout.rs` 的真实宽高比），点键帽看当前绑定，可改成键盘键、多媒体键、鼠标键，或恢复出厂默认 |
| 灯效 | 亮度 / 速度各 1–6 档、颜色候选；**不改模式编号**（模式名与编号的对照表官方没公开） |
| 宏 | 列出槽位与动作；录制与写入未做（两段式写入，风险最高，留最后） |
| 高级键 | 从键位表反查 SOCD / MT / TGL / CB / DKS / MPT（这些没有独立命令） |
| 设置 | 回报率（只读，2.4G 下改可能断链）、按键延时、休眠档位、NKRO |

### 一条设计约束：不长期持有设备句柄

`hidapi::HidDevice` 不适合跨线程存活，而 `AppView` 会被 GPUI 实体要求可发送。
所以每个操作都是「打开 → 做事 → 关闭」，在后台执行器里跑完再把结果送回界面。
打开是毫秒级，用户无感，换来的是不用处理句柄生命周期与重连。

### 开发用启动参数

```bash
OPENMOUSE_START_PAGE=keyboard OPENMOUSE_KB_TAB=settings ./openmouse-gpui.exe
OPENMOUSE_START_PAGE=keyboard OPENMOUSE_KB_SELFTEST=key_delay ./openmouse-gpui.exe
```

`OPENMOUSE_KB_TAB` 取 `keys|lighting|macro|advanced|settings`。`OPENMOUSE_KB_SELFTEST`
会在读完数据后真的走一次「改设置」写入（按键延时在 3 和 4 之间来回），用的是和点
界面 chip 完全相同的 `kb_edit_settings`，用来回归验证写入链路——合成鼠标点击在
DPI 缩放下坐标不可靠，这条路更稳。

## 写入验证记录（真机）

| 检查 | 方式 | 结果 |
|---|---|---|
| 设置原值写回 | `write_test settings` | 56 字节写回后逐字节一致 |
| 键位表原值写回 | `write_test keymap` | 128 个键位写回后一致 |
| 单键改写 + 还原 | `write_test change` | 键位 105（PgUp→Home）写入生效，只有它变化，还原后 512 字节一致 |
| 设置字段改写 + 还原 | `write_test settings-change` | 按键延时 3→4，只有 `[4]` 变化，还原后一致 |
| 界面发起的写入 | `OPENMOUSE_KB_SELFTEST` | 日志「按键延时 3 → 4」「写入成功」，设备读回 `[4]=0x04`，再跑一次回到 3 |

**写回是无损的**：`GameMode` / `LedEffect` / `SideLight` 内部保留读回来的原始字节，
`encode()` 以它为底只覆盖已建模字段。原因是这几段数据里有很多没名字的厂内保留位
（设置 56 字节里有 30 多个，侧灯 24 字节里有 15 个），拿全零重建再写回去等于把它们
清掉。有单测 `unmodelled_bytes_survive_a_write_back` 守住这点。

## 待办

- 宏的录制与写入（两段式：先 400 字节索引表，再数据区，顺序错了会写坏）。
- GIF 灯效与音乐律动（大块传输，单包最大 4104 字节，还没做）。
- 灯效模式编号 ↔ 名称对照表：官方在懒加载语言包里，静态抓不全，需要真机逐档试探。
- 回报率写入：等有线环境验证再开放（2.4G 下可能断链）。
- TFT / 磁轴相关模块对 NUT75 不适用，已确认跳过。
- `openmouse-keyboard` 直接用了 `hidapi`，与 `openmouse-hid` 的封装并存。
  两边各持有独立的 `HidApi` 句柄，互不影响，但长远看应该收敛成一套。
