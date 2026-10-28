# 中文化说明（本地改动）

界面文案已全部改为简体中文。改动只涉及源码，不在 `node_modules` 里，所以 `npm ci` 不会冲掉。

## 改了哪三个文件

| 文件 | 内容 |
|---|---|
| `gpui/src/app.rs` | 主界面全部文案（约 240 处），并新增 `zh_state()` 显示映射函数 |
| `gpui/src/tray.rs` | 托盘菜单「打开 OpenMouse」 |
| `gpui/js/controls.ts` | 驱动控件文案表（`ZH`）+ `localizeSection()`，约 210 条 |

## 一个关键设计：逻辑值不翻译

设备的连接方式、LOD、表面模式、灯效模式这些值**同时用于代码里的相等判断**，例如：

```rust
r.connection.as_deref() == Some("Wireless")
```

直接把字符串改成中文会破坏判断。所以这些值保持英文原样，只在**显示时**经 `zh_state()` 转成中文：

```rust
"Wireless" => "无线",
"Onboard"  => "板载",
"Host"     => "软件控制",
"Low" => "低", "Medium" => "中", "High" => "高",
"Off" => "关闭", "Static" => "常亮", "Breathing" => "呼吸",
```

JS 侧同理：`localizeSection()` 只改 `title / label / description / options[].label / zeroLabel`，
以及 `kind: "info"` 的 `current`。`choice`、`stepped` 的 `current` 是提交值兼选中态依据，一律不动。

## 没翻译的部分

这些保持英文是刻意的，不是遗漏：

- **发布说明正文**（Updates 页里 v0.0.1 下面的条目）— 属于内容不是界面，且会随版本更新变化
- **品牌与产品名** — Razer、Logitech、Endgame Gear、Ninjutso、Finalmouse、Starlight X、Shard、Pulsar 等
- **游戏名** — League of Legends 等
- **技术缩写** — DPI、CPI、USB、Hz、ID、GX、TMR、SmartShift、LightForce、HyperClick、Perfect Polling
- **protocol 库里未出现在 `controls.ts` 的少数文案** — 各品牌驱动内部的长尾控件，若界面出现英文，
  往 `controls.ts` 的 `ZH` 表里加一行即可，不用改驱动库

## 想再加词条

编辑 `gpui/js/controls.ts` 里的 `const ZH = { ... }`，然后重新编译：

```bash
node gpui/js/build.mjs          # 只需重打 JS
cd gpui && cargo build --release
```

`ZH` 里没有的键会原样返回，所以加词条不会影响未收录的文案。

## 回滚成英文

```bash
cd openmouse-desktop
cp gpui/src/app.rs.en.bak  gpui/src/app.rs
cp gpui/src/tray.rs.en.bak gpui/src/tray.rs
# controls.ts 需要手动删除 ZH 表、localizeSection 和 describeControls 里的 .map(localizeSection)
cd gpui && cargo build --release
```

## 字体

没有改字体设置。GPUI 的 `.SystemUIFont` 在 Windows 上走 DirectWrite 回退，
中文由系统字体（微软雅黑）渲染，实测字形正常、无缺字方块。macOS 上会走苹方。

## 额外改动：精简侧边栏与移除更新体系

侧边栏底部原是三行（更新 / 帮助与反馈 / 设置），现只剩「设置」一行，外层的圆角边框框保留。

### 1. 删除「帮助与反馈」入口

`gpui/src/app.rs` 的 `render_sidebar()` 里：

- 删掉 `footer_row("footer-discord", ...)` 的 `.child(...)` 和它上面的一条分隔线
- 删掉不再使用的 `let discord = img(themed_discord(...))` 变量
- 彻底删除 `DISCORD_INVITE_URL`、`DISCORD_SVG`、`themed_discord()` 三个符号
- 删除资源文件 `gpui/assets/discord.svg`

### 2. 删除整个更新体系

| 删除内容 | 位置 |
|---|---|
| 侧边栏「更新 v0.0.1 · 发布说明」入口 | `app.rs` `render_sidebar()` |
| `Page::Updates` 枚举及其 `label()` / `icon()` 分支 | `app.rs` |
| `render_updates()`（版本卡片 + 发布说明时间线 + 检查更新按钮） | `app.rs` |
| `note_update_available()` / `check_for_updates_now()` | `app.rs` |
| 字段 `updates_list` / `update_available` / `checking_for_updates` | `app.rs` |
| `PageColors`、`release_row()`、`UPDATES_LEADING_ROWS` | `app.rs` |
| `last_notified_update_version`（4 处） | `settings.rs` |
| `spawn_update_watcher()` 调用 + `OPENMOUSE_START_PAGE` 的 `updates` 分支 | `bin/main.rs` |
| `pub mod changelog;` / `pub mod update_check;` | `lib.rs` |
| `changelog.rs`、`update_check.rs`、`js/lib/changelog.ts` | 整文件删除 |
| `build.mjs` 里生成 `dist/changelog.json` 的步骤 | `js/build.mjs` |

现在应用**完全不联网检查版本**，也不再显示发布说明。标题栏的 `v0.0.1 · Release` 版本徽标保留（读 `CARGO_PKG_VERSION`，不依赖更新模块）。

旧 `settings.json` 里遗留的 `last_notified_update_version` 键不会报错——`StoredSettings` 没有开
`deny_unknown_fields`，多余键直接被忽略。

### 顺带清理

删除过程中 `repaint_theme()` 被误删（它夹在 `check_for_updates_now` 和 `render_updates` 之间），
已从 `app.rs.en.bak` 原样恢复。另外清掉了三个编译警告：`Collapsible` / `Disableable` 未使用导入、
`render()` 里未使用的 `pages` 变量。

最终编译只剩一个 `could not embed the app icon` 警告（中文路径导致，先前就有）。
