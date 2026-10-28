Same root cause on another PID — and the `SUPPORTED_HID_FILTERS` workaround will not fix it

Built from `main` @ `fd98d25`, Windows 11 26H2 (build 26300). Device: Razer Viper V2 Pro,
`1532:00a6` (wireless, stock HyperPolling receiver).

Overview shows "No supported mouse found", and `openmouse-gpui.exe --list-supported out.json`
writes `[]`.

Cause is the merged-collection one, not a missing filter entry:

- `0x00a6` **already has** an entry in `SUPPORTED_HID_FILTERS`
  (`RAZER_VIPER_V2_CONTROL_FILTERS` in `dist/drivers/vendors.js`, usagePage `0x01` / usage `0x02`),
  so `preferredCollectionFor()` does return `{0x01, 0x02}` for this device.
- It still fails, because `candidatesForDevice()` in `gpui/js/native-hid/brands.ts` falls into
  the empty branch:

  ```ts
  const eligible = claimed.length > 0
    ? claimed
    : info.collections && info.collections.length > 0
      ? []                                   // ← lands here
      : all.filter((c) => brandServesVendor(c.brand, info.vendorId));
  ```

  `claimed` is empty: `RazerHidClient.isSupported()` needs `device.collections` to be *exactly one*
  Generic Desktop Mouse collection, while `hid_list_interfaces()` merges every split of the
  `(vendorId, productId)` group into one interface. Each split reports `1 collection(s)` in the
  log; merged, this device ends up with 12.

So adding the PID to `SUPPORTED_HID_FILTERS` is not sufficient by itself — the single-collection
gate in `isSupported()` has to tolerate the merged shape. Worth knowing before going down that
road for `0x009c` as well.

### Verified local fix

Patched `dist/drivers/razer/hid.js`:

```js
function isMouseControlInterface(device) {
  const collections = device.collections ?? [];
  return collections.some((c) => c?.usagePage === 0x01 && c?.usage === 0x02);
}
```

On real hardware this brings the device back:

- `--list-supported` → `[{"name":"Razer Viper V2 Pro","brand":"Razer"}]`
- Overview → "1 supported device found", battery 71%, Wireless (i.e. a real `readStatus()`)
- Performance tab reads DPI 1600, polling-rate presets 125/500 Hz, lift-off distance, Link X/Y

`hid_open()` still routes to the right split through `preferredCollection`, so accepting the
collection anywhere in the group does not seem to address the wrong interface — but I only
exercised reads and the DPI panel, no writes.

### Two things worth keeping in mind

- This is not two PIDs. Every Razer model whose control channel is the single Generic Desktop
  Mouse collection is affected — the `RAZER_VIPER_V2_*`, `RAZER_VIPER_V3_*`, `RAZER_VIPER_*` and
  `RAZER_VIPER_MINI_*` filter groups. Models on a vendor collection (`>= 0xFF00`) are unaffected,
  because `hasVendorCollection()` still matches the merged array.
- The cleaner fix is probably in the transport (keep the per-split collections and expose which
  split carries the preferred collection) rather than relaxing `isSupported()` in the protocol,
  since relaxing it there would also change browser behaviour.
