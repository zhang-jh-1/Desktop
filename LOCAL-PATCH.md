# 本地补丁说明（非上游代码）

## 解决了什么

OpenMouse Desktop 不识别 **Razer Viper V2 Pro (1532:00a6)**，界面显示
"No supported mouse found"。

根因：`crates/openmouse-hid/src/lib.rs` 的 `hid_list_interfaces()` 按
`(vendorId, productId)` 把复合设备的所有分片合并成一个接口，
`interface_collections()` 把所有分片的 collection 全部 merge 进一个数组；
而 `@openmouse/protocol` 的雷蛇驱动 `isMouseControlInterface()` 要求
`collections` **恰好只有一个** Generic Desktop Mouse collection。
合并后是 12 个，判断失败 → 没有驱动认领 → 候选列表为空。

## 改了什么

只改一个文件（node_modules 里，**不在 git 版本控制内**）：

```
node_modules/@openmouse/protocol/dist/drivers/razer/hid.js
```

`isMouseControlInterface()` 从"必须恰好一个 collection"改成"组内任意位置有
0x01/0x02 的 collection 即可"。打开设备时仍由 `preferredCollection`（来自
`RAZER_VIPER_V2_CONTROL_FILTERS` 的 0x01/0x02）路由到正确分片。

补丁文件：`local-patch-razer-collections.patch`
原始备份：`node_modules/@openmouse/protocol/dist/drivers/razer/hid.js.orig`

## 重新应用

`npm ci` / `npm install` 会覆盖 node_modules，补丁会丢失。重新执行 `npm ci` 之后：

```bash
cd node_modules/@openmouse/protocol/dist/drivers/razer
patch -p0 < ../../../../local-patch-razer-collections.patch   # 路径按实际调整，或用下一条
```

更保险的做法是直接手工编辑 `hid.js`，把 `isMouseControlInterface` 函数体换成：

```js
function isMouseControlInterface(device) {
    const collections = device.collections ?? [];
    return collections.some((collection) => collection?.usagePage === 0x01 && collection?.usage === 0x02);
}
```

然后重新编译（build.rs 的 `rerun-if-changed` 覆盖了 protocol 目录，会自动重新打包 JS）：

```bash
cd gpui && cargo build --release
```

## 回滚

```bash
cd node_modules/@openmouse/protocol/dist/drivers/razer
cp hid.js.orig hid.js
cd -
cd gpui && cargo build --release
```

## 验证过的行为

- `openmouse-gpui.exe --list-supported out.json` → `[{"name":"Razer Viper V2 Pro","brand":"Razer"}]`
- Overview 页显示 "1 supported device found"，电量 71%，Wireless（真实 `readStatus()`）
- 设备页读到 DPI 1600、轮询率 125/500 Hz 预设、Lift-off distance、Link X/Y 开关

## 注意事项

- 只验证了**读取**（readStatus、DPI 面板）。**未验证写入**——没动过 DPI/轮询率的实际下发。
- 影响范围：所有控制通道依赖单一 Generic Desktop Mouse collection 的雷蛇型号
  （Viper V2 Pro / V3 Pro / Viper / Viper Mini 那一组过滤规则）。
- 上游 issue #28 记录的是同一个根因（DeathAdder V2 X HyperSpeed），
  已在 https://github.com/OpenMouse-Project/Desktop/issues/28#issuecomment-6462604220
  补充了 Viper V2 Pro 的复现与验证结果。
