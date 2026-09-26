# XiTieDiary v1 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 v1 日记应用：Windows + Android，本地 SQLite + 用户自带 OSS/S3 的 Joplin 式同步。

**Architecture:** Svelte 5 UI 经 Tauri commands 调用全 Rust 数据层（rusqlite + OpenDAL 同步引擎），凭证不进 WebView，同步引擎用 OpenDAL fs 后端做双客户端单元测试。

**Tech Stack:** Tauri 2 · Svelte 5 + Vite + TS · pnpm · rusqlite(bundled) · Apache OpenDAL · image · vitest

**Spec:** `docs/superpowers/specs/2026-09-26-xitiediary-v1-design.md`

## Global Constraints

- 零密钥入库：`local.json` 已被 gitignore；任何输出（日志/错误/事件）中 secret 只显示前 4 字符
- UI 文案中文；代码标识符英文；不加注释除非必要
- Rust：stable 1.98，crates 走 rsproxy（已全局配置）；pnpm 走 npmmirror（已配置）
- 每个 Task 结束：`cargo test` 与 `pnpm test`（配置后）全绿才可 commit
- 提交信息用 conventional commits（feat:/fix:/test:/docs:/chore:）
- Tauri 配置：`productName: XiTieDiary`，`identifier: com.xitie2000.xitiediary`
- 日期一律 `YYYY-MM-DD` 字符串；时间戳一律 unix epoch 毫秒（i64）

## Review Focus

1. **超大图片**（≥4000px）：压缩后长边 ≤1920、JPEG mime、体积显著缩小 → Task 8 `compress_resizes_large_image`
2. **首次同步/空桶**：有数据的设备全量上传且不误删；空设备全量下载 → Task 12 `first_sync_uploads` / `bootstrap_downloads`
3. **双端并发编辑同一 entry**：产生 `[冲突]` 副本，双方内容都保留 → Task 12 `conflict_creates_copy_preserving_both`
4. **墓碑/孤儿媒体传播**：A 端删除后 B 端软删且列表不显示；媒体失去引用后软删 → Task 12 `tombstone_propagates` / `orphan_media_soft_deleted`
5. **配置缺失/坏 JSON/凭证错误**：返回 `{code, message}` 优雅错误，不 panic 不 crash → Task 3 `missing_config_is_error` + Task 13 `sync_error_emits_event`

---

### Task 1: Tauri 项目脚手架

**Files:**
- Create: `src/`（svelte-ts 模板）、`src-tauri/`、`package.json`、`vite.config.ts`、`tsconfig.json`
- Modify: `.gitignore`（合并模板条目与现有密钥条目）

**Interfaces:**
- Produces: 可运行的 `pnpm tauri dev` 空窗口；`src-tauri/src/lib.rs` 中的 `run()`；`App.svelte` 骨架

- [ ] **Step 1: 在临时目录生成模板再合入仓库根**

```powershell
# 仓库根已有 .git/.gitignore/local.example.json/docs，create-tauri-app 要求空目录，故先到临时目录生成
cd $env:TEMP\opencode; pnpm create tauri-app scaffold --template svelte-ts --manager pnpm --identifier com.xitie2000.xitiediary --yes
# 然后把 scaffold/* 移动到 D:\opencode\XiTieDiary\（含点文件），.gitignore 内容合并进现有文件
```
模板名以 `pnpm create tauri-app --help` 实际输出为准（参数可能微调）。

- [ ] **Step 2: 安装依赖并验证 dev 可跑**

Run: `pnpm install`（npmmirror）；`pnpm tauri dev`
Expected: 窗口打开显示模板页。注意 cargo 走 rsproxy；首次编译约 5–10 分钟。

- [ ] **Step 3: `pnpm build` 前端类型检查通过**

Run: `pnpm build` → vite build + svelte-check 无错误。

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "chore: scaffold tauri 2 + svelte 5 (svelte-ts template)"
```

### Task 2: 数据层 db.rs（schema + Entry/Media/remote_state CRUD）

**Files:**
- Create: `src-tauri/src/db.rs`、`src-tauri/src/types.rs`
- Test: `src-tauri/src/db.rs` 内 `#[cfg(test)]` 模块（rusqlite in-memory）

**Interfaces:**
- Produces:
  - `types.rs`：`Entry { id: String, date: String, content: String, created_at: i64, updated_at: i64, deleted: bool }`（serde Serialize/Deserialize，snake_case）；`MediaMeta { id, entry_id, mime, size, updated_at, deleted }`；`RemoteState { key: String, etag: Option<String>, updated_at: Option<i64> }`
  - `db.rs`：`Db::open_in_memory() -> Result<Db>`、`Db::open(path: &Path) -> Result<Db>`（建 schema：spec §4 三张表 + 两个索引，WAL + foreign_keys）
  - `list_entries(from: Option<&str>, to: Option<&str>) -> Vec<Entry>`（date 倒序，过滤 deleted）
  - `get_entry(id) -> Option<Entry>`；`upsert_entry(&Entry)`；`soft_delete_entry(id) -> Option<Entry>`（置 deleted=1 并 bump updated_at）
  - `insert_media(&MediaMeta)`；`list_media(entry_id) -> Vec<MediaMeta>`（过滤 deleted）；`list_all_media() -> Vec<MediaMeta>`（含 deleted，同步用）；`soft_delete_media(id)`；`link_media(id, entry_id)`
  - `get_remote_state(key) -> Option<RemoteState>`；`set_remote_state(&RemoteState)`；`list_remote_state() -> Vec<RemoteState>`
- Consumes: 无（独立）

- [ ] **Step 1: 写失败测试**（`db.rs` test 模块）

测试名与断言要点：
- `schema_creates_tables_and_indexes`（含 `PRAGMA user_version` == 1，spec §9 迁移机制）
- `upsert_then_list_entries_orders_by_date_desc_and_hides_deleted`
- `soft_delete_entry_sets_flag_and_bumps_updated_at`
- `media_crud_roundtrip_and_link`
- `remote_state_upsert_roundtrip`

- [ ] **Step 2: 跑测试确认失败** — Run: `cargo test --manifest-path src-tauri/Cargo.toml`，Expected: 编译失败（模块不存在）

- [ ] **Step 3: 实现 db.rs + types.rs** — rusqlite `bundled`；UUID 由调用方生成（`uuid::Uuid::new_v4()`），db 层不生成；`deleted` 列 INTEGER 0/1 与 bool 手动转换

- [ ] **Step 4: 跑测试确认通过** — `cargo test` 全绿

- [ ] **Step 5: Commit** — `git commit -m "feat: sqlite data layer with entries/media/remote_state"`

### Task 3: 配置加载 config.rs

**Files:**
- Create: `src-tauri/src/config.rs`
- Test: 同文件 `#[cfg(test)]`

**Interfaces:**
- Produces: `SyncConfig { provider, endpoint, region, bucket, prefix, access_key_id, access_key_secret: String }`；`AppError`（见 Task 4 前置，此处先用 `ConfigError` 变体）；`load_config(explicit: Option<&Path>) -> Result<SyncConfig, ConfigError>`
- 查找顺序：参数路径 > `XITIEDIARY_CONFIG` 环境变量 > `./local.json`（cwd）> `app_data_dir/local.json`（由调用方传入路径，config.rs 不依赖 tauri）
- 环境变量逐字段覆盖：`XITIEDIARY_PROVIDER/ENDPOINT/REGION/BUCKET/PREFIX/ACCESS_KEY_ID/ACCESS_KEY_SECRET`
- `ConfigError`：`Missing`、`Malformed(String)`、`Incomplete(String /*缺哪个字段*/)`；实现 `Display`，secret 值不进错误文本

- [ ] **Step 1: 失败测试** — `missing_config_is_error`（三个查找点都不存在 → Missing）；`parses_local_json`；`env_overrides_json`（XITIEDIARY_BUCKET 覆盖）；`empty_bucket_is_incomplete`；`malformed_json_is_error`
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现** — serde 反序列化 + 环境变量覆盖 + 校验非空字段（provider ∈ {oss,s3,cos,webdav}）
- [ ] **Step 4: 确认通过** — `cargo test`
- [ ] **Step 5: Commit** — `git commit -m "feat: sync config loading with env overrides"`

### Task 4: Tauri commands + 错误映射

**Files:**
- Create: `src-tauri/src/commands.rs`、`src-tauri/src/error.rs`
- Modify: `src-tauri/src/lib.rs`（注册 commands，管理 `Db` 为 `tauri::State`，app_data_dir 打开 `diary.db`）

**Interfaces:**
- Produces（全部 `#[tauri::command]`，异步无效操作直接同步）：
  - `list_entries(state, from: Option<String>, to: Option<String>) -> Result<Vec<Entry>, AppErrorDto>`
  - `get_entry(state, id: String) -> Result<Option<Entry>, AppErrorDto>`
  - `create_draft_entry(state, date: String) -> Result<Entry, AppErrorDto>`（content=""，立即落库）
  - `save_entry(state, id: String, date: String, content: String) -> Result<Entry, AppErrorDto>`（bump updated_at）
  - `delete_entry(state, id: String) -> Result<(), AppErrorDto>`（软删 + 关联媒体软删）
  - `insert_media(state, entry_id: String, path: String) -> Result<MediaMeta, AppErrorDto>`（path 为前端 dialog 选中的文件路径；压缩见 Task 8，本 Task 先原样落盘+落库）
  - `delete_media(state, id: String) -> Result<(), AppErrorDto>`
  - `get_config_status(state) -> Result<ConfigStatus, AppErrorDto>`，`ConfigStatus { configured: bool, provider: Option<String>, bucket: Option<String> }`（不含密钥）
  - `AppError { Db, Config(ConfigError), Io, Sync(String) }`（thiserror）；`AppErrorDto { code: String, message: String }`，From 映射：db→"db"、config→"config"、io→"io"、sync→"sync"
- Draft 清理规则（编辑器关闭时调用）：`cleanup_empty_drafts(state)` — content 为空、无媒体、且 `remote_state` 无该 entry 键 → 物理删除该行

- [ ] **Step 1: 失败测试** — `error.rs` 测试：`app_error_dto_codes`（四个变体的 code 断言）；`config_error_message_hides_secret`（构造含 secret 的错误，断言 Display 不含完整 secret）
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现 error.rs + commands.rs + lib.rs 装配**（commands 保持薄：取 State<Db>，调用 db.rs/Task 8 的函数，错误 From 转换；`media` 文件存 `app_data_dir/media/{id}.jpg`）
- [ ] **Step 4: 确认通过 + `pnpm tauri dev` 手动冒烟**（浏览器 console invoke list_entries 返回 `[]`）
- [ ] **Step 5: Commit** — `git commit -m "feat: tauri commands with typed error mapping"`

### Task 5: 前端数据层 api.ts + stores

**Files:**
- Create: `src/lib/api.ts`、`src/lib/stores.svelte.ts`、`src/lib/types.ts`
- Test: `src/lib/api.test.ts`（vitest + vi.mock('@tauri-apps/api/core')）

**Interfaces:**
- Produces: api.ts 导出与 Rust commands 一一对应的类型化函数（`listEntries(from?: string, to?: string): Promise<Entry[]>` 等，`invoke<AppErrorDto>` 错误 throw `Error(message, {cause: code})`）；`src/lib/types.ts` 复用 `Entry/MediaMeta` 接口（手写 TS interface，字段与 Rust serde 输出一致）
- stores.svelte.ts：`entries = $state<Entry[]>([])`、`loadEntries()`、`syncStatus = $state<'idle'|'syncing'|'ok'|'error'>('idle')`
- Consumes: Task 4 的 command 名称与 DTO 形状

- [ ] **Step 1: vitest 基础设施** — `pnpm add -D vitest @testing-library/svelte jsdom`，`package.json` scripts 加 `"test": "vitest run"`
- [ ] **Step 2: 失败测试** — `api_calls_invoke_with_correct_command_and_args`（mock invoke，断言 command 名与参数）；`api_throws_with_code_on_error_dto`
- [ ] **Step 3: 实现 api.ts + stores**
- [ ] **Step 4: `pnpm test` 通过**
- [ ] **Step 5: Commit** — `git commit -m "feat: typed frontend api layer and stores"`

### Task 6: 列表视图 + App 布局

**Files:**
- Create: `src/lib/components/EntryList.svelte`
- Modify: `src/App.svelte`（顶部 SyncBar 占位 + 新建按钮 + 列表/编辑器切换）、`src/app.css`（极简中文 UI 样式）

**Interfaces:**
- Produces: `EntryList.svelte` props：`entries: Entry[]`，事件 `onselect(id: string)`；按 date 分组倒序，组头显示 `M月D日 周X`，条目显示 content 前 50 字与图片数角标
- Consumes: Task 5 stores；视图切换 `view = $state<{page:'list'} | {page:'editor', id: string}>`

- [ ] **Step 1: 失败测试** — `EntryList` 渲染测试：`renders_date_groups_desc`、`shows_preview_and_media_count`（@testing-library/svelte，构造两条不同日期 entry）
- [ ] **Step 2: 确认失败** — `pnpm test`
- [ ] **Step 3: 实现组件与 App 布局**（新建按钮 → `createDraftEntry(今天)` → 跳编辑器）
- [ ] **Step 4: `pnpm test` + `pnpm tauri dev` 手动看列表空态**
- [ ] **Step 5: Commit** — `git commit -m "feat: entry list grouped by date with app shell"`

### Task 7: 编辑器视图（草稿生命周期 + 自动保存）

**Files:**
- Create: `src/lib/components/EntryEditor.svelte`
- Modify: `src/App.svelte`（路由接入）

**Interfaces:**
- Produces: `EntryEditor.svelte` props：`entryId: string`，事件 `onclose()`；textarea 输入 debounce 800ms 调 `saveEntry`；关闭时内容为空走 `cleanupEmptyDrafts`；显示 date（可改，`<input type="date">`）与媒体缩略图（Task 8 接入）

- [ ] **Step 1: 失败测试** — `typing_debounces_save`（vi.useFakeTimers：两次输入只触发一次 saveEntry，800ms 后）；`close_calls_cleanup_when_empty`
- [ ] **Step 2: 确认失败** — `pnpm test`
- [ ] **Step 3: 实现**（媒体区先留占位 div）
- [ ] **Step 4: `pnpm test` 通过 + dev 手动新建/编辑/关闭草稿**
- [ ] **Step 5: Commit** — `git commit -m "feat: entry editor with draft lifecycle and debounced autosave"`

### Task 8: 图片链路（压缩 + 插入 + Windows 选图）

**Files:**
- Create: `src-tauri/src/images.rs`
- Modify: `src-tauri/src/commands.rs`（`insert_media` 走压缩）、`Cargo.toml`（`image` crate）、`src/lib/components/EntryEditor.svelte`（"+"按钮：`@tauri-apps/plugin-dialog` open filters images）、`lib.rs` 注册 dialog 插件
- Test: `src-tauri/src/images.rs` 测试模块

**Interfaces:**
- Produces: `images.rs`：`compress_to_jpeg(bytes: &[u8], max_edge: u32, quality: u8) -> Result<(Vec<u8>, u32, u32), AppError>`（长边 >max_edge 缩放；输出 JPEG q=quality；调用于 max_edge=1920, quality=80）
- Consumes: Task 4 `insert_media` 签名不变，内部改为：读文件 → compress → 写 `media/{id}.jpg` → insert MediaMeta（mime 固定 image/jpeg）

- [ ] **Step 1: 失败测试** — `compress_resizes_large_image`（用 image crate 生成 4000×2500 纯色 PNG → 压缩 → 断言 ≤1920 长边、解码 mime 为 JPEG、字节数 < 输入的一半）；`compress_keeps_small_image_dims`（800×600 → 尺寸不变）
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现 images.rs + command 集成 + 前端 dialog 选图与缩略图**（缩略图 = `<img>` + CSS，路径经 `convertFileSrc`）
- [ ] **Step 4: `cargo test` + `pnpm test` + dev 手动插一张大图验证**
- [ ] **Step 5: Commit** — `git commit -m "feat: image pipeline with 1920px jpeg compression and dialog picking"`

### Task 9: M1 打包验证

**Files:**
- Modify: `src-tauri/tauri.conf.json`（窗口标题 XiTieDiary、默认尺寸 420×720）

- [ ] **Step 1: `pnpm tauri build`** 产出 NSIS 安装包，记录 exe/msi 体积（预期 6–10MB，写进 commit message）
- [ ] **Step 2: 安装并冒烟**（新建条目、贴图、重启数据仍在）
- [ ] **Step 3: Commit** — `git commit -m "chore: m1 desktop packaging verified (NN MB)"`

---

### Task 10: 同步远端封装 remote.rs

**Files:**
- Create: `src-tauri/src/sync/remote.rs`（`sync/mod.rs` 空模块占位）
- Modify: `Cargo.toml`（opendal，features：`services-oss, services-s3, services-cos, services-webdav, services-fs`）
- Test: remote.rs 测试模块

**Interfaces:**
- Produces: `Remote::new(cfg: &SyncConfig, root: &Path) -> Result<Remote, AppError>`（provider=`fs` 时 root 作为桶根，其余按 cfg 构建 Operator，全部加 `layers::TimeoutLayer`）
  - `list(&self) -> Vec<(String /*key*/, Option<String> /*etag*/)>`（key 形如 `entries/{id}.json`，已去 prefix；fs 后端 etag 用文件 mtime+size 拼）
  - `get_entry(&self, id) -> Result<Bytes>`；`put_entry(&self, id, body: &[u8]) -> Result<()>`
  - `get_media(&self, id) -> Result<Bytes>`；`put_media(&self, id, body: &[u8]) -> Result<()>`
  - `write_lock(device: &str, ts: i64)`、`read_lock() -> Option<LockInfo { device, ts }>`、`delete_lock()`、`refresh_lock(device, ts)`
- Consumes: Task 3 `SyncConfig`

- [ ] **Step 1: 失败测试** — `fs_roundtrip_put_list_get`（put entry+media → list 返回两个 key → get 内容一致）；`lock_write_read_delete`；`fs_etag_changes_on_overwrite`
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现**（oss/s3/cos/webdal builder 分支 + fs 分支；OpenDAL list 的 etag 字段后端不一致时统一 `Option`）
- [ ] **Step 4: 确认通过** — `cargo test`
- [ ] **Step 5: Commit** — `git commit -m "feat: opendal remote wrapper with lock helpers"`

### Task 11: 同步判定 protocol.rs（纯逻辑）

**Files:**
- Create: `src-tauri/src/sync/protocol.rs`
- Test: protocol.rs 测试模块

**Interfaces:**
- Produces:
  - `RemoteEntry`：与 `Entry` 同字段的 wire 类型，serde **忽略未知字段**（前向兼容）
  - `enum EntryAction { Ignore, Download, Upload, Conflict { keep_local: bool } }`
  - `decide_entry(local: Option<Entry>, last_synced: Option<i64>, remote: Option<RemoteEntry>) -> EntryAction`
    规则（spec §5）：无 remote→Upload；无 local→Download；本地净（local.updated_at==last_synced）→ remote 较新则 Download 否则 Ignore；本地脏且远端也变（remote.updated_at≠last_synced）→ updated_at 大者胜、败者入 `Conflict{keep_local}`；本地脏远端未变→Upload
  - `enum MediaAction { Skip, Download, Upload, LocalTombstone }`
  - `decide_media(local: Option<&MediaMeta>, referenced: bool, on_remote: bool) -> MediaAction`
- Consumes: Task 2 `Entry/MediaMeta`

- [ ] **Step 1: 失败测试** — `local_only_uploads`、`remote_only_downloads`、`clean_local_ignores_older_remote`、`dirty_local_uploads_when_remote_unchanged`、`both_changed_lww_and_conflict`（含 keep_local 两个方向）、`tombstone_json_roundtrip`（deleted=true 序列化/反序列化）、`unknown_fields_ignored`（JSON 加 `"future":1` 仍解析成功）
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现**（纯函数无 IO）
- [ ] **Step 4: 确认通过** — `cargo test`
- [ ] **Step 5: Commit** — `git commit -m "feat: sync decision functions with conflict lww semantics"`

### Task 12: 同步引擎 mod.rs（编排 + 锁 + 双客户端场景测试）

**Files:**
- Create: `src-tauri/src/sync/mod.rs`（实现）、`src-tauri/src/sync/tests.rs`
- Modify: `src-tauri/src/lib.rs`（mod sync）

**Interfaces:**
- Produces: `pub async fn run_sync(db: &Db, remote: &Remote, media_dir: &Path, device: &str) -> Result<SyncReport, AppError>`
  - `SyncReport { downloaded_entries: u32, uploaded_entries: u32, downloaded_media: u32, uploaded_media: u32, conflicts: u32 }`
  - 编排：锁（TTL 5 分钟，过期可覆盖，结束删除）→ list → 对每个 entry 走 `decide_entry`（Conflict 时先把败者内容写成新 entry：id=新 UUID、content=`[冲突 {原日期}] {败者.content}`、其余字段同败者）→ 媒体按 `decide_media` 传文件 → 孤儿媒体软删 → 全程写 `remote_state` → 释放锁
  - 远端媒体对象**永不删除**（spec §12 #4 取舍）
- Consumes: Task 10 `Remote`、Task 11 判定函数、Task 2 db CRUD

- [ ] **Step 1: 失败测试**（tests.rs：tempdir 建 fs 远端 + 两个 in-memory Db，`fn client(tag) -> (Db, Path)` 工具）
  - `first_sync_uploads`（Review Focus #2）
  - `bootstrap_downloads`（Review Focus #2）
  - `bidirectional_merge`（A 加 e2、B 加 e3，各同步一次再互同步，双方都有两条）
  - `conflict_creates_copy_preserving_both`（Review Focus #3：同一 entry 双端改，断言胜者内容在位 + `[冲突]` 副本存在且两份内容都可检索）
  - `tombstone_propagates`（Review Focus #4：A 软删→同步→B 同步→B 的 list_entries 为空）
  - `orphan_media_soft_deleted`（Review Focus #4：A 保存去掉媒体的 entry→同步→B 同步→B 媒体行 deleted=1）
  - `stale_lock_is_overwritten`（写 ts=now-10min 的锁，同步成功）
  - `fresh_lock_skips_sync`（ts=now 的他设备锁 → Err(sync) 且数据无变化）
- [ ] **Step 2: 确认失败** — `cargo test`
- [ ] **Step 3: 实现引擎**（tokio::fs 传输媒体；entry JSON serde_json::to_vec）
- [ ] **Step 4: 确认通过** — `cargo test` 全绿（本 Task 是全项目测试密度最高处）
- [ ] **Step 5: Commit** — `git commit -m "feat: sync engine with best-effort lock and conflict copies"`

### Task 13: sync command + SyncBar UI + 启动同步

**Files:**
- Modify: `src-tauri/src/commands.rs`（`sync_now`、事件 `sync://status`）、`src/lib/components/SyncBar.svelte`、`src/lib/stores.svelte.ts`、`src/App.svelte`（onMount 自动 sync）
- Test: `SyncBar.test.ts`

**Interfaces:**
- Produces: `#[tauri::command] async fn sync_now(app, state) -> Result<SyncReport, AppErrorDto>`（配置缺失时返回 config 错误码而非 panic）；事件负载 `{ status: 'syncing'|'ok'|'error', report?: SyncReport, message?: string }`（message 脱敏）
- Consumes: Task 12 `run_sync`、Task 4 `get_config_status`

- [ ] **Step 1: 失败测试** — `sync_error_emits_event`（Review Focus #5：无配置时 mock 事件 emit，断言 status=error、message 不含 secret 占位）；`SyncBar` 渲染四种状态文案（同步中/已同步/失败/idle）
- [ ] **Step 2: 确认失败** — `cargo test` + `pnpm test`
- [ ] **Step 3: 实现**（tauri::Emitter emit；前端 `listen('sync://status')` 更新 store）
- [ ] **Step 4: 确认通过** — 全部测试绿
- [ ] **Step 5: Commit** — `git commit -m "feat: manual and startup sync with status bar events"`

### Task 14: 配置模板更新 + OSS 真实联通 + 文档

**Files:**
- Modify: `local.example.json`（spec §6 的 8 字段，删除 `STS_AUTH_ENDPOINT`）、`README.md`（创建：简介、截图位、local.json 配置说明、误提交凭证必须轮换的警示、构建指南）

- [ ] **Step 1: 更新 local.example.json 为 spec §6 字段**
- [ ] **Step 2: 用户在 OSS 控制台建桶（私有读写、按用户配置），本地 local.json 填入真实凭证，dev 模式插一条数据 → sync_now → OSS 控制台对象浏览器确认 `entries/{id}.json` 与 `media/{id}` 存在**（此步需要用户配合提供桶，实现者只跑验证）
- [ ] **Step 3: 第二台设备语义验证**：清空 app_data_dir 重新启动 → 启动同步 → 数据回来
- [ ] **Step 4: Commit** — `git commit -m "docs: oss setup guide and updated config template"`

---

### Task 15: Android 初始化 + debug APK

**Files:**
- Create: `src-tauri/gen/android/`（`tauri android init` 生成）
- Modify: `lib.rs` 无需改；`src-tauri/tauri.conf.json` 无需改（identifier 已合规）

**Interfaces:**
- Produces: 可构建的 Android 工程；`pnpm tauri android build --debug --target aarch64` 产出 APK

- [ ] **Step 1: `pnpm tauri android init`**（需要 ANDROID_HOME/NDK_HOME/JAVA_HOME 环境变量已在 M0 配好；新终端验证 `echo $env:ANDROID_HOME`）
- [ ] **Step 2: `pnpm tauri android build --debug --target aarch64`** 产出 `gen/android/app/build/outputs/apk/*/debug/*.apk`（首次 gradle 下载走 Google Maven，dl.google.com 可达）
- [ ] **Step 3: 模拟器或真机安装冒烟**（`adb install -r *.apk`，列表/新建/编辑可用）
- [ ] **Step 4: Commit** — `git add src-tauri/gen/android` 相关源文件（按 tauri 官方 gitignore 惯例，构建产物不入库）+ `git commit -m "feat: android platform init with debug apk"`

### Task 16: Android 选图 spike（+ 条件回退）

**Files:**
- Modify: `src/lib/components/EntryEditor.svelte`（桌面路径 dialog 之外，加 `<input type="file" accept="image/*">` 仅在 Android 显示——`platform() === 'android'`）

**Interfaces:**
- Produces: spike 结论记录到 `docs/superpowers/notes/2026-09-android-image-pick.md`（可行/不可行、证据截图位）
- 判定标准：真机/模拟器点"+" → 系统相册选择器弹出 → 选图后 `File.arrayBuffer()` → `invoke('insert_media')`... 注意 input file 是二进制不是路径，需新 command `insert_media_bytes(entry_id, bytes: Vec<u8>)`（与 Task 8 共用压缩管线）

- [ ] **Step 1: 增加 `insert_media_bytes` command（复用 Task 8 压缩），cargo test 补 `bytes_pipeline_matches_file_pipeline`**
- [ ] **Step 2: 编辑器双通道（Android input / 桌面 dialog），pnpm test 更新**
- [ ] **Step 3: APK 安装真机测选图**，写 spike 结论文档
- [ ] **Step 4:（仅当 spike 失败）回退任务**：`src-tauri/plugins/image-picker`（Kotlin `ACTION_GET_CONTENT` + ActivityResult，command `pick_image() -> String?`）——单独 commit
- [ ] **Step 5: Commit** — `git commit -m "feat: android image picking via webview file input (spike result)"`

### Task 17: M3 验收 + README 完成版

- [ ] **Step 1: release APK**：`pnpm tauri android build --target aarch64`（签名用 debug key 占位即可，正式签名留给发布日），记录体积
- [ ] **Step 2: 真机全流程验收**：新建/编辑/贴图/删除 → 手动同步 → Windows 端同步看到变化（双向）；断网编辑 → 恢复 → 收敛一致
- [ ] **Step 3: README 完成**（功能、体积实测数据、双端构建指南、LICENSE 建议 MIT——需用户确认）
- [ ] **Step 4: CI**：`.github/workflows/ci.yml`（windows runner：`cargo test` + `pnpm test` + `pnpm build`；tauri build 不进 CI，本地/发布时跑）
- [ ] **Step 5: Commit** — `git commit -m "docs: v1 complete with verified android release"`
