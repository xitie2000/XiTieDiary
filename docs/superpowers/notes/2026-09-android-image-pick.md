# Android 选图 spike 结论（2026-09-26）

## 问题

Tauri 2 官方插件没有跨平台图片选择器，Android 端如何选图？

## 方案

WebView 原生 `<input type="file" accept="image/*" multiple>`——零插件。

## 代码级证据（已完成）

1. **wry 0.55.1（Tauri 的 Android WebView 封装）已实现 `onShowFileChooser`**
   （`wry-0.55.1/src/android/kotlin/RustWebChromeClient.kt:272`）：相册选择器、
   相机拍摄（含权限申请分支）均已处理。点击 input 会唤起系统文件选择器。
2. **传输管线**：JS `FileReader.readAsDataURL` → base64 字符串 →
   `invoke('insert_media_bytes')` → Rust `base64::decode` → 与桌面路径
   `insert_media`（dialog 选路径 → fs::read）共用同一 `write_media`
   （压缩 1920px JPEG q80 → 落盘 → 落库）。
   单测 `bytes_pipeline_matches_file_pipeline` 断言两条管线产出逐字节一致的 JPEG ✓
3. **前端**：`EntryEditor.svelte` 按 `navigator.userAgent` 分流——
   Android 点「＋ 图片」触发隐藏 input；桌面走 `tauri-plugin-dialog`。
   组件测试 `file_input_routes_to_insert_media_bytes` ✓

## 真机确认（待用户）

`pnpm tauri android build --debug --target aarch64` 产物：
`src-tauri/gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk`

安装到手机（`adb install -r <apk>` 或直接传文件安装），验证：
- [ ] 点「＋ 图片」弹出系统相册选择器
- [ ] 选图后缩略图出现
- [ ] 同步后 Windows 端能看到该图

若失败 → 回退方案：自写 ~100 行 Kotlin 插件（`ACTION_GET_CONTENT` + ActivityResult），
见计划 Task 16 Step 4。

## 已知取舍

- base64 传输有 33% 体积膨胀；当前压缩前原图典型 2–5MB，传输字符串 ~3–7MB，
  可接受。若真机验证发现卡顿，后续可换 Tauri raw IPC（`InvokeBody::Raw`）。
- debug APK 约 631MB（Rust debug 符号未剥离），release 体积见 T17 实测。
