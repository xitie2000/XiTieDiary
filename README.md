# XiTieDiary

轻量的本地优先日记应用 —— Windows / Android，数据在你自己的云存储里。

## 特性

- 纯文本 + 图片日记，按日期浏览
- 本地优先：离线完全可用，SQLite 存储
- 云同步：Joplin 式对象存储同步（阿里云 OSS / AWS S3 / 腾讯云 COS / WebDAV），**自带凭证、零自建服务器**
- 冲突不丢数据：双端并发修改时自动保留新版本并生成 `[冲突]` 副本
- 图片自动压缩（长边 ≤1920px JPEG）

## 实测体积（v0.1.0）

| 平台 | 产物 | 体积 |
|---|---|---|
| Windows | NSIS 安装包 | 2.55 MB |
| Windows | MSI 安装包 | 3.55 MB |
| Windows | 裸 exe | 7.41 MB |
| Android | release APK（arm64） | 18.4 MB |

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

> **配置文件位置**：开发时放仓库根目录（`pnpm tauri dev` 的查找点）；安装版在应用数据目录
> （Windows：`%APPDATA%\com.xitie2000.xitiediary\local.json`）。**Android 端无需手动放置**：
> 首次点同步栏的「导入配置」按钮，选择你的 `local.json` 即可，导入后自动开始同步。

> **安全警示**：`local.json` 永远不要提交到仓库。如果误提交了凭证，仅删除提交是不够的——**必须立刻到云控制台轮换（禁用并重建）该 AccessKey**。

## 构建

工具链：Node.js 22+ / pnpm / Rust stable / VS Build Tools（Windows 桌面）；Android 还需 Android Studio（SDK 36 + NDK r28）+ JDK 21。

```powershell
pnpm install
pnpm tauri dev                      # 开发运行（桌面）
pnpm tauri build                    # 打包 Windows 安装包
pnpm tauri android init             # 首次：生成 Android 工程
pnpm tauri android build --target aarch64  # 打包 Android APK
```

> 国内网络提示：`gen/android/gradle/wrapper/gradle-wrapper.properties` 与
> `gen/android/build.gradle.kts` 已配置腾讯/阿里云镜像，如在海 外网络可自行换回官方源。

## TODO（待修复的网络问题）

开发过程中因国内网络受限遇到并绕过的问题，后续有条件时建议回退到官方渠道：

- [ ] **rustls-platform-verifier Kotlin 组件**（`gen/android/app/src/main/java/org/rustls/platformverifier/`）
  官方通过 Gradle 从 GitHub Maven 仓库拉取 AAR（`raw.githubusercontent.com/rustls/rustls-platform-verifier/android/`），
  GitHub 不可达时无法使用。当前用手写的 Kotlin 实现替代（用系统 `X509TrustManager` 校验证书链），
  并加了 ProGuard keep 规则防 R8 裁剪。网络恢复后建议换回官方 AAR（版本号见 `Cargo.lock` 中
  `rustls-platform-verifier-android`）。
- [ ] **Gradle 依赖仓库**（`gen/android/build.gradle.kts` + `buildSrc/build.gradle.kts`）
  已插入阿里云镜像（gradle-plugin / google / public），官方源（`plugins.gradle.org` /
  `maven.google.com`）不可达导致 `kotlin-compiler-embeddable` 等下载失败。海外构建时可删掉镜像行回退。
- [ ] **Gradle 发行版**（`gen/android/gradle/wrapper/gradle-wrapper.properties`）
  `services.gradle.org` 下载超时（60% 处断连），已换腾讯镜像
  `mirrors.cloud.tencent.com/gradle/`。海外环境可改回官方 URL。
- [ ] **GitHub HTTPS 推送偶发失败**（`schannel: failed to receive handshake, SSL/TLS connection failed`）
  `git push` 间歇性握手失败，重试即可成功。如频繁出现考虑配置 git 代理
  （`git config --global http.proxy socks5://...`）。
- [ ] **Android Release APK 签名**：当前使用 debug keystore 占位
  （`~/.android/debug.keystore`），正式发布前需生成专用签名密钥
  （`keytool -genkeypair` + 配置 `gen/android/keystore.properties`）。

## 开发

```powershell
pnpm test      # 前端 vitest
cargo test --manifest-path src-tauri/Cargo.toml   # Rust 单测（含同步引擎双客户端场景）
cargo test --manifest-path src-tauri/Cargo.toml --test oss_live -- --ignored  # 真实 OSS 手动验证（需 local.json）
```

同步协议的设计与已知取舍见 `docs/superpowers/specs/2026-09-26-xitiediary-v1-design.md`。

## 许可证

[MIT](LICENSE)
