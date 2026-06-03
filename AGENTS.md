# AGENTS.md

## 文档语言

所有输出的文档（specs、plans、README、代码注释等）均使用中文。

## 开发

```bash
npm install
npm run tauri dev       # 开发模式，热更新
npm run tauri build     # 生产构建
```

## 架构

- **前端** `src/` — HTML/CSS/JS，通过 Tauri IPC（`invoke`/`listen`）与 Rust 通信
- **后端** `src-tauri/src/` — Rust 模块
  - `lib.rs` — Tauri 配置、command 注册
  - `state.rs` — 应用状态（文件列表、模板）
  - `builder.rs` — hadris-cd 封装
  - `templates.rs` — 模板 JSON 持久化

## 非显式约定

- 模板持久化到 `~/.iso_maker_templates.json`
- 构建进度通过 `iso-progress` 事件推送
- ISO 路径规范化：大写、非法字符替换为 `_`
