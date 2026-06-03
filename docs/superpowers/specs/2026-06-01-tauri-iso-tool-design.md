# Tauri ISO Tool — Design Spec

## Overview

Port the Python/Flask ISO Maker to a pure-Rust Tauri v2 desktop app. Frontend (HTML/CSS/JS) is reused and ported; backend replaces Flask + pycdlib with Tauri IPC + hadris-cd.

## Architecture

Single-process Tauri app. Frontend communicates with Rust backend exclusively through Tauri IPC (`invoke` / `listen`). No HTTP server, no subprocess.

```
Webview (HTML/CSS/JS)  ←→  Tauri Commands + Events  ←→  hadris-cd (ISO building)
                                                       ←→  Template Store (JSON file)
```

## Key Design Decisions

- **Tauri native IPC** — commands for request/response, events for streaming progress (replaces REST + SSE).
- **Tauri dialog for file selection** — `@tauri-apps/plugin-dialog` opens native file/folder pickers, returns paths. No drag-and-drop for v1.
- **ISO building in `spawn_blocking`** — synchronous hadris-cd writes happen off the event loop, progress pushed back via `app_handle.emit()`.
- **Templates stored as JSON** — `~/.iso_maker_templates.json`, same format as Python version for data compatibility.

## Components

### Frontend (`src/`)

Port existing `static/index.html`, `static/style.css`, `static/app.js` into Tauri's `src/` directory.

Changes from Python version:

| Python (old)              | Tauri (new)                                        |
|---------------------------|----------------------------------------------------|
| `fetch('/api/upload', ..)`| `invoke('add_files', {paths: [...]})`               |
| `fetch('/api/upload', fd)`| removed (no HTTP multipart)                         |
| `EventSource('/api/progress')`| `listen('iso-progress', callback)`              |
| `fetch('/api/create-iso')`| `invoke('build_iso', {volumeLabel, files, outputPath})` |
| `fetch('/api/download')`  | `invoke('save_iso_dialog')` → native save dialog    |
| `fetch('/api/templates')` | `invoke('get_templates')` / `invoke('save_template')` / `invoke('delete_template')` |
| `fetch('/api/browse')`    | removed (use Tauri dialog instead)                   |
| `fetch('/api/default-output')`| `invoke('get_default_output')`                   |

Drop zone replaced with file-picker buttons using `@tauri-apps/plugin-dialog`.

### Rust Backend (`src-tauri/`)

#### Commands (in `lib.rs`)

| Command                  | Input                         | Output                        |
|--------------------------|-------------------------------|-------------------------------|
| `add_files`             | `Vec<String>` (paths)         | `Vec<FileEntry>` (name/iso_path/size) |
| `remove_file`           | `usize` (index)               | —                              |
| `build_iso`             | `BuildRequest {volume_label, files, output_path}` | `task_id: String` |
| `get_templates`         | —                             | `Vec<Template>`                |
| `save_template`         | `Template`                    | —                              |
| `delete_template`       | `String` (name)               | —                              |
| `get_default_output`    | —                             | `String` (path)                |
| `pick_output_path`      | —                             | `String` (dialog result)       |

#### State management

- `AppState` wrapped in `Mutex<...>`, managed by `tauri::State`.
- Holds: `Vec<FileEntry>` (current file list), template store, output path.

#### ISO Builder module (`builder.rs`)

Wraps hadris-cd:

```rust
pub fn build_iso(
    volume_label: &str,
    files: &[FileEntry],
    output_path: &Path,
    progress_cb: impl Fn(usize, usize, &str),
) -> Result<()>
```

Calls `CdWriter::new(file, options).write(tree)` with options: ISO 9660 + Joliet + Rock Ridge, `interchange_level` 3.

Progress callback fires once per file added.

#### Template Store (`templates.rs`)

CRUD over `~/.iso_maker_templates.json`. `serde_json` for serialization. Mutex-protected in-memory cache + persistence.

### Event flow: ISO building

1. Frontend calls `invoke('build_iso', request)`
2. Command returns `task_id` immediately, spawns `tokio::task::spawn_blocking`
3. Worker builds ISO, calls `app_handle.emit("iso-progress", {...})` per file
4. On completion: `app_handle.emit("iso-progress", {status: "done", output_path, size})`
5. On error: `app_handle.emit("iso-progress", {status: "error", message})`

### Event types

```rust
#[derive(Serialize, Clone)]
struct ProgressEvent {
    status: String,       // "working" | "done" | "error"
    current: usize,
    total: usize,
    filename: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}
```

## Dependencies

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
hadris-cd = "1.1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
```

## File Structure

```
Tauri_ISO_tool/
├── src/                          # Frontend
│   ├── index.html
│   ├── styles.css
│   └── main.js
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── src/
│   │   ├── main.rs               # Entry point
│   │   ├── lib.rs                 # Tauri setup, command definitions
│   │   ├── builder.rs             # hadris-cd wrapper
│   │   ├── templates.rs           # Template CRUD
│   │   └── state.rs               # AppState, FileEntry types
│   └── icons/
└── docs/superpowers/specs/
    └── 2026-06-01-tauri-iso-tool-design.md
```

## Error Handling

- All commands return `Result<T, String>`. Rust errors mapped to user-facing messages.
- File-not-found, permission-denied, and hadris-cd errors all surfaced through the progress event error path.
- Failed ISO builds leave no partial output files.

## Scope (v1)

In scope:
- Add files via native file/folder dialog
- File list with name, ISO path, size display
- Volume label and output filename configuration
- ISO building with real-time progress
- Auto-download on completion via native save dialog
- Template save/load/delete

Out of scope:
- Drag-and-drop (complex with Tauri v2)
- File browsing sidebar (replaced by dialog)
- Multi-session / append-to-existing-ISO
- El-Torito bootable ISO creation (hadris-cd supports it, but not in v1 UI)
# Tauri ISO Tool — 设计方案

## 概述

将 Python/Flask 实现的 ISO Maker 迁移为纯 Rust Tauri v2 桌面应用。前端（HTML/CSS/JS）复用并适配；后端用 Tauri IPC + hadris-cd 替代 Flask + pycdlib。

## 架构

单进程 Tauri 应用。前端通过 Tauri IPC（`invoke` / `listen`）与 Rust 后端通信，无 HTTP 服务器，无子进程。

```
Webview (HTML/CSS/JS)  ←→  Tauri Commands + Events  ←→  hadris-cd（ISO 构建）
                                                       ←→  模板存储（JSON 文件）
```

## 关键设计决策

- **Tauri 原生 IPC** — commands 处理请求/响应，events 推送流式进度（替代 REST + SSE）。
- **Tauri dialog 选文件** — `@tauri-apps/plugin-dialog` 弹出原生文件/文件夹选择器，返回路径。v1 不做拖拽。
- **ISO 构建跑在 `spawn_blocking`** — 同步的 hadris-cd 写入操作脱离事件循环，进度通过 `app_handle.emit()` 推回前端。
- **模板用 JSON 文件存储** — `~/.iso_maker_templates.json`，与 Python 版格式一致，数据兼容。

## 组件

### 前端（`src/`）

将现有 `static/index.html`、`static/style.css`、`static/app.js` 迁移到 Tauri 的 `src/` 目录。

与 Python 版的差异：

| Python（旧）              | Tauri（新）                                        |
|---------------------------|----------------------------------------------------|
| `fetch('/api/upload', ..)`| `invoke('add_files', {paths: [...]})`               |
| `fetch('/api/upload', fd)`| 移除（无 HTTP multipart）                           |
| `EventSource('/api/progress')`| `listen('iso-progress', callback)`              |
| `fetch('/api/create-iso')`| `invoke('build_iso', {volumeLabel, files, outputPath})` |
| `fetch('/api/download')`  | `invoke('save_iso_dialog')` → 原生保存对话框        |
| `fetch('/api/templates')` | `invoke('get_templates')` / `invoke('save_template')` / `invoke('delete_template')` |
| `fetch('/api/browse')`    | 移除（改用 Tauri dialog）                            |
| `fetch('/api/default-output')`| `invoke('get_default_output')`                   |

拖拽区域替换为使用 `@tauri-apps/plugin-dialog` 的文件选择按钮。

### Rust 后端（`src-tauri/`）

#### Commands（定义在 `lib.rs`）

| Command                  | 输入                         | 输出                        |
|--------------------------|-------------------------------|-------------------------------|
| `add_files`             | `Vec<String>`（文件路径）      | `Vec<FileEntry>`（name/iso_path/size） |
| `remove_file`           | `usize`（索引）                | —                              |
| `build_iso`             | `BuildRequest {volume_label, files, output_path}` | `task_id: String` |
| `get_templates`         | —                             | `Vec<Template>`                |
| `save_template`         | `Template`                    | —                              |
| `delete_template`       | `String`（模板名）             | —                              |
| `get_default_output`    | —                             | `String`（路径）               |
| `pick_output_path`      | —                             | `String`（对话框结果）         |

#### 状态管理

- `AppState` 用 `Mutex<...>` 包裹，通过 `tauri::State` 管理。
- 持有：`Vec<FileEntry>`（当前文件列表）、模板存储、输出路径。

#### ISO 构建模块（`builder.rs`）

封装 hadris-cd：

```rust
pub fn build_iso(
    volume_label: &str,
    files: &[FileEntry],
    output_path: &Path,
    progress_cb: impl Fn(usize, usize, &str),
) -> Result<()>
```

调用 `CdWriter::new(file, options).write(tree)`，选项：ISO 9660 + Joliet + Rock Ridge，`interchange_level` 3。

每添加一个文件触发一次进度回调。

#### 模板存储（`templates.rs`）

对 `~/.iso_maker_templates.json` 的增删改查。`serde_json` 序列化。Mutex 保护的内存缓存 + 持久化。

### 事件流：ISO 构建

1. 前端调用 `invoke('build_iso', request)`
2. Command 立即返回 `task_id`，启动 `tokio::task::spawn_blocking`
3. 工作线程构建 ISO，每个文件完成后调用 `app_handle.emit("iso-progress", {...})`
4. 完成时：`app_handle.emit("iso-progress", {status: "done", output_path, size})`
5. 出错时：`app_handle.emit("iso-progress", {status: "error", message})`

### 事件类型

```rust
#[derive(Serialize, Clone)]
struct ProgressEvent {
    status: String,       // "working" | "done" | "error"
    current: usize,
    total: usize,
    filename: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
}
```

## 依赖

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
tauri-plugin-dialog = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
hadris-cd = "1.1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
```

## 文件结构

```
Tauri_ISO_tool/
├── src/                          # 前端
│   ├── index.html
│   ├── styles.css
│   └── main.js
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/default.json
│   ├── src/
│   │   ├── main.rs               # 入口
│   │   ├── lib.rs                 # Tauri 配置、command 定义
│   │   ├── builder.rs             # hadris-cd 封装
│   │   ├── templates.rs           # 模板增删改查
│   │   └── state.rs               # AppState、FileEntry 类型
│   └── icons/
└── docs/superpowers/specs/
    └── 2026-06-01-tauri-iso-tool-design.md
```

## 错误处理

- 所有 command 返回 `Result<T, String>`。Rust 错误映射为用户可读消息。
- 文件不存在、权限拒绝、hadris-cd 错误均通过进度事件的 error 路径上报。
- ISO 构建失败不残留不完整的输出文件。

## 范围（v1）

在范围内：
- 通过原生文件/文件夹对话框添加文件
- 文件列表显示名称、ISO 路径、大小
- 卷标和输出文件名配置
- ISO 构建及实时进度
- 完成后通过原生保存对话框自动下载
- 模板保存/加载/删除

不在范围内：
- 拖拽添加（Tauri v2 实现复杂）
- 文件浏览侧栏（改为对话框）
- 追加写入已有 ISO
- El-Torito 可启动 ISO（hadris-cd 支持，v1 不做 UI）

