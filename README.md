# XiTieDiary

轻量的本地优先日记应用 —— Windows / Android，数据在你自己的云存储里。

## 特性

- 纯文本 + 图片日记，按日期浏览
- 本地优先：离线完全可用，SQLite 存储
- 云同步：Joplin 式对象存储同步（阿里云 OSS / AWS S3 / 腾讯云 COS / WebDAV），**自带凭证、零自建服务器**
- 冲突不丢数据：双端并发修改时自动保留新版本并生成 `[冲突]` 副本
- 图片自动压缩（长边 ≤1920px JPEG），桌面安装包约 2.5MB

## 同步原理

每条日记是一个 JSON 对象（`entries/{id}.json`），每张图片是一个二进制对象（`media/{id}`），存放在你自己配置的对象存储桶里。同步协议：

- 下行：列举远端对象（增量，仅拉取 ETag 变化的条目）
- 上行：本地较新则上传；删除以墓碑形式传播
- 冲突：`updated_at` 大者胜，败者保存为冲突副本

## 配置同步

复制 `local.example.json` 为 `local.json`（已被 .gitignore 忽略），填入你自己的云存储凭证：

```json
{
  "provider": "oss",
  "endpoint": "https://oss-cn-hangzhou.aliyuncs.com",
  "region": "cn-hangzhou",
  "bucket": "你的桶名",
  "prefix": "xitiediary",
  "access_key_id": "你的AK",
  "access_key_secret": "你的SK"
}
```

`provider` 支持：`oss`（阿里云）、`s3`（AWS 及兼容）、`cos`（腾讯云）、`webdav`。

所有字段也可以用环境变量 `XITIEDIARY_*` 覆盖（如 `XITIEDIARY_BUCKET`）。

> **安全警示**：`local.json` 永远不要提交到仓库。如果误提交了凭证，仅删除提交是不够的——**必须立刻到云控制台轮换（禁用并重建）该 AccessKey**。

## 构建

工具链：Node.js 22+ / pnpm / Rust stable / VS Build Tools（Windows 桌面）；Android 还需 Android Studio（SDK 36 + NDK r28）。

```powershell
pnpm install
pnpm tauri dev            # 开发运行（桌面）
pnpm tauri build          # 打包 Windows 安装包
pnpm tauri android build  # 打包 Android APK（先 pnpm tauri android init）
```

## 许可证

待定（计划 MIT）。
