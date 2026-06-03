# Tauri ISO Tool 实施计划

> **目标：** 将 Python ISO Maker 迁移为纯 Rust Tauri v2 桌面应用

**架构：** Tauri 单进程，前端通过 IPC（invoke/listen）与 Rust 后端通信；hadris-cd 构建 ISO，spawn_blocking 避免阻塞 UI

**技术栈：** Tauri v2、Rust 2021、hadris-cd 1.1、vanilla HTML/CSS/JS、@tauri-apps/plugin-dialog

---

### 任务 1: 配置 Rust 依赖和 Tauri 插件

**文件：**
- 修改：`src-tauri/Cargo.toml`
- 修改：`src-tauri/tauri.conf.json`
- 修改：`src-tauri/capabilities/default.json`

- [ ] **Step 1: 添加 Rust 依赖**

```bash
cd src-tauri && cargo add hadris-cd@1.1 tokio --features full && cargo add uuid --features v4 && cargo add tauri-plugin-dialog@2
```
预期：`Cargo.toml` 中新增 `hadris-cd`、`tokio`、`uuid`、`tauri-plugin-dialog` 依赖。

- [ ] **Step 2: 更新 tauri.conf.json**

将窗口标题改为 `ISO Maker`，窗口宽度改为 `900`、高度改为 `700`。

- [ ] **Step 3: 更新 capabilities/default.json 添加 dialog 权限**

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Capability for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "opener:default",
    "dialog:default",
    "dialog:allow-open",
    "dialog:allow-save"
  ]
}
```

- [ ] **Step 4: 编译验证**

---

### 任务 2: 应用状态和类型定义

**文件：**
- 创建：`src-tauri/src/state.rs`

定义 `FileEntry`、`BuildRequest`、`ProgressEvent`、`Template`、`AppState` 五个结构体。

---

### 任务 3: 模板存储模块

**文件：**
- 创建：`src-tauri/src/templates.rs`

实现 `load_templates()`、`save_template()`、`delete_template()` 三个函数，读写 `~/.iso_maker_templates.json`。

---

### 任务 4: ISO 构建模块

**文件：**
- 创建：`src-tauri/src/builder.rs`

封装 hadris-cd：`build_iso()` 构建 ISO、`estimate_size()` 估算大小、`sanitize_volume_label()` 规范化卷标。

---

### 任务 5: 注册 Tauri Commands

**文件：**
- 修改：`src-tauri/src/lib.rs`

注册 10 个 command：`add_files`、`get_files`、`remove_file`、`clear_files`、`build_iso`、`estimate_iso_size`、`get_default_output`、`get_templates`、`save_template`、`delete_template`。

---

### 任务 6: 移植前端

**文件：**
- 替换：`src/index.html`
- 替换：`src/styles.css`
- 替换：`src/main.js`

将原项目的 HTML/CSS/JS 适配为 Tauri IPC 模式，`fetch` → `invoke`，`EventSource` → `listen`。

---

### 任务 7: 编译修复和集成验证

- [ ] 编译检查 `cargo check`
- [ ] 开发模式启动 `npm run tauri dev`
- [ ] 功能验证：选择文件 → 构建 ISO → 成功
- [ ] 提交代码
