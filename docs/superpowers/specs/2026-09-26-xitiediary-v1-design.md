# XiTieDiary v1 设计文档

- 日期：2026-09-26
- 状态：设计已获用户认可，待实施
- 仓库：https://github.com/xitie2000/XiTieDiary

## 1. 背景与目标

个人日记应用，开源发布于 GitHub。核心诉求：

1. **多端**：Windows + Android（iOS/macOS 明确排除在 v1 之外）
2. **体积小**：系统 WebView 方案，桌面目标 6–10MB、Android 目标 10–15MB
3. **本地优先**：离线完全可用，云端同步不依赖自建服务器
4. **开源安全**：仓库零密钥，用户自带云存储凭证

## 2. 范围

### v1 功能

- 日记条目 CRUD：纯文本 + 多张图片，按日期（`YYYY-MM-DD`）组织
- 条目列表按日期倒序分组浏览，点击进入编辑
- 云端同步：启动时自动 + 手动触发；冲突不丢数据
- UI 中文

### 非目标（v1 明确不做）

iOS/macOS 平台、Markdown 渲染、标签、全文搜索、多用户、端到端加密、富文本、自动后台同步。

## 3. 总体架构与技术栈

```
┌──────────────┐  Tauri IPC   ┌───────────────────────┐
│ Svelte 5 UI  │ ───────────▶ │ Rust 数据层            │
│ (WebView)    │  invoke()    │ ├ commands.rs  对外 API │
└──────────────┘              │ ├ db.rs        SQLite  │
                              │ ├ config.rs    配置    │
                              │ └ sync/        同步引擎 │
                              └────┬──────────┬───────┘
                                   │          │
                              diary.db    OSS/S3/COS
                              media/      (OpenDAL)
```

| 层 | 选型 | 理由 |
|---|---|---|
| 壳 | Tauri 2 stable | 体积最小的跨平台方案 |
| 前端 | Svelte 5 + Vite + TypeScript | 编译型框架，运行时最小 |
| 包管理 | pnpm（npmmirror registry） | 速度快、磁盘省 |
| 数据库 | rusqlite（bundled SQLite，WAL） | 纯 Rust 访问，无并发分层 |
| 对象存储 | Apache OpenDAL（oss/s3/cos/webdav/fs/memory 服务） | 一份代码多后端；`fs`/`memory` 用于单测 |
| 图片 | image crate | 解码/压缩 |
| 异步 | tokio + serde + uuid | 标准组合 |

**分层原则**：凭证与签名只存在于 Rust 进程，不进 WebView；前端不直接发 SQL、不直接碰云存储。

## 4. 数据模型

```sql
CREATE TABLE entries (
  id         TEXT PRIMARY KEY,     -- UUID v4
  date       TEXT NOT NULL,        -- 'YYYY-MM-DD'（本地时区）
  content    TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,     -- unix epoch 毫秒
  updated_at INTEGER NOT NULL,
  deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE media (
  id         TEXT PRIMARY KEY,     -- UUID v4，同时是文件名/对象名
  entry_id   TEXT NOT NULL,
  mime       TEXT NOT NULL,        -- 统一 image/jpeg
  size       INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  deleted    INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE remote_state (        -- 同步清单：远端已知状态缓存
  key        TEXT PRIMARY KEY,     -- 'entries/{id}' 或 'media/{id}'
  etag       TEXT,
  updated_at INTEGER
);
CREATE INDEX idx_entries_date ON entries(date);
CREATE INDEX idx_media_entry  ON media(entry_id);
```

- 开启 `PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;`
- media 二进制存 `app_data_dir/media/{id}.jpg`，数据库只存元数据
- 图片插入时压缩：长边 ≤1920px、JPEG quality 80，**统一转 JPEG**（v1 不支持动图）
- 删除条目 = 软删（`deleted=1`），UI 过滤；媒体随之软删

## 5. 同步协议（Joplin 简化版）

### 远端布局（用户自己的桶）

```
{prefix}/entries/{id}.json   条目 JSON：{id,date,content,created_at,updated_at,deleted}
{prefix}/media/{id}          图片二进制（同 id 内容永久不变）
{prefix}/lock                同步锁
```

### 流程

1. **加锁**：PUT `lock`（`{"device":"<uuid>","ts":<now>}`），TTL 5 分钟，超时可删——**尽力而为**，不保证原子
2. **下行**：LIST `{prefix}/` 得到全部 (key, etag) → 与 `remote_state` 比对 → etag 变化或未知 → GET 解析
   - entry：远端 `updated_at` 较新 → 覆盖本地；本地较新 → 留待上行
   - media：id 不可变，etag 不同 → 下载覆盖本地文件
   - 远端有而本地无 → 直接下载（新条目/新设备）
3. **上行**：本地 `updated_at` > 远端已知值（或远端无）→ PUT；删除以墓碑形式 PUT（`deleted:true` 的 JSON）
4. **删除传播**：设备拉到墓碑后本地执行软删；孤儿媒体（不再被任何 entry JSON 引用的 media 记录）在下行应用后同步软删
5. **冲突**：某条目本地已改 且 远端 etag 也变 → 保留 `updated_at` 新者；旧者复制为内容前缀 `[冲突]` 的新条目（新 UUID，仅本地，下次同步上行）——**不丢数据，多一个副本**
6. **更新 manifest**：写回 `remote_state`，释放锁

### 触发与呈现

- 启动后自动同步一次 + UI 手动同步按钮（v2 再做编辑后 debounce 自动同步）
- 同步跑在 tokio 后台任务，UI 通过 Tauri event 收进度/结果
- 网络/凭证错误 → toast 提示，不 crash，本地数据不受影响

## 6. 配置与安全

`local.json`（已 gitignore）字段：

```json
{
  "provider": "oss",            // oss | s3 | cos | webdav
  "endpoint": "",
  "region": "",
  "bucket": "",
  "prefix": "xitiediary",
  "access_key_id": "",
  "access_key_secret": ""
}
```

- 加载优先级：环境变量 `XITIEDIARY_*` > `local.json`
- 仓库内提交 `local.example.json` 模板（仅字段名）；需移除旧的 `STS_AUTH_ENDPOINT` 字段
- 日志与错误信息脱敏：secret 仅显示前 4 字符
- README 注明：误提交凭证后删除提交不够，必须立刻在云控制台轮换密钥

## 7. 图片处理与选图

| 平台 | 方案 |
|---|---|
| Windows | `tauri-plugin-dialog` 选文件（图片过滤器） |
| Android | **首选** WebView 原生 `<input type="file" accept="image/*">`（M3 spike 验证）；**兜底**自写 ~100 行 Kotlin Tauri 插件（`ACTION_GET_CONTENT`） |

图片处理（Rust，`image` crate）：解码 → 缩放（长边 ≤1920px）→ JPEG q80 → 存 `media/{id}.jpg`。列表页用 CSS 缩放显示（v1 不生成真实缩略图）。

## 8. 项目结构

```
XiTieDiary/
├── src/                       # Svelte 前端
│   ├── lib/
│   │   ├── api.ts             # invoke() 类型化封装
│   │   ├── stores/            # entries/sync 状态
│   │   └── components/        # EntryList / EntryEditor / SyncBar …
│   ├── App.svelte
│   └── app.css
├── src-tauri/
│   ├── src/
│   │   ├── lib.rs             # 插件注册 + command 注册
│   │   ├── commands.rs        # #[tauri::command] 对外 API
│   │   ├── db.rs              # schema + CRUD
│   │   ├── config.rs          # local.json / 环境变量加载
│   │   └── sync/
│   │       ├── mod.rs         # 引擎入口（run_sync）
│   │       ├── protocol.rs    # 冲突/墓碑/锁（纯逻辑，可测）
│   │       └── remote.rs      # OpenDAL Operator 封装
│   ├── Cargo.toml
│   └── tauri.conf.json
├── docs/superpowers/specs/
├── local.example.json
└── .gitignore
```

**commands API（v1）**：`list_entries(date_range?)` / `get_entry(id)` / `save_entry(entry)` / `delete_entry(id)` / `insert_media(entry_id, bytes) -> media_id` / `sync_now()` / `get_sync_config_status()`。

## 9. 错误处理

- Rust 层：`thiserror` 定义错误枚举，统一序列化为 `{code, message}` 给前端；凭证/网络错误有独立 code
- 前端：api 层 catch → toast；同步失败保留本地修改，下次重试
- 数据库迁移：`PRAGMA user_version` 版本号，v1 只有一版

## 10. 测试策略

- **Rust 单测（重点）**：`sync::protocol` 用 OpenDAL `fs`/`memory` 后端构造"远端 + 两个客户端"，覆盖：初次同步、双向同步、冲突产生副本、墓碑传播、锁过期抢占
- **Rust**：`db.rs` CRUD 测试（内存数据库）
- **前端**：vitest + @testing-library/svelte，列表渲染、编辑保存冒烟测试（mock api 层）
- **CI**：GitHub Actions（windows runner）`cargo test` + `pnpm build` + `tauri build`
- Android 构建验证：v1 手动，后续补 CI

## 11. 里程碑与验收

| | 内容 | 验收标准 |
|---|---|---|
| M0 | 工具链 | ✅ 2026-09-26 完成 |
| M1 | 脚手架 + 桌面 CRUD + 选图 | Windows 安装包可用，日常记录无障碍 |
| M2 | 同步引擎 + OSS 联通 | 单测全绿；两台设备数据一致，断网可写、联网收敛 |
| M3 | Android 适配 | 真机 APK 跑通 CRUD + 同步 + 选图 |

## 12. 已知限制与风险

| # | 限制/风险 | 对策 |
|---|---|---|
| 1 | Android 选图无官方插件（最大不确定项） | M3 先 spike `input[type=file]`，兜底自写插件 |
| 2 | OSS 锁非原子，过期抢占有竞态 | 尽力而为锁 + 冲突副本兜底（不丢数据） |
| 3 | 时钟偏移影响 LWW 判定 | 冲突副本机制保底；NTP 提示留给 v2 |
| 4 | 墓碑永久累积 | 个人量级（<10 万对象）无害，接受 |
| 5 | 体积估算是估计值 | M1/M3 实测后写进 README，不夸大宣传 |

## 13. 决策记录

| 决策 | 备选 | 理由 |
|---|---|---|
| Tauri 2 | Flutter（~15MB）/ RN / MAUI | 体积硬指标；UI 简单不需要自绘引擎 |
| Svelte 5 | Vue 3 / React / 原生 TS | 运行时最小、编译型 |
| 全 Rust 数据层 | tauri-plugin-sql + 前端同步 | 凭证不进 WebView；同步引擎可单测 |
| 用户自带凭证 | STS 授权服务 | 开源零密钥零服务器；未来多用户再演进 |
| OpenDAL | 手写 OSS 签名/reqwest | 统一多后端；fs/memory 便于测试；Apache 顶级项目 |
| rusqlite | tauri-plugin-sql (sqlx) | 单层访问无并发问题；同步引擎同语言 |
| Joplin 式同步 | 自建 REST 服务端 | 零服务器成本，个人多设备场景够用 |
| 砍 iOS/macOS | 四端全做 | 风险 1/5/6 全部消除；后续按需加回 |
