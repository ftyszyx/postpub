# Postpub Root Rebuild Architecture

## Summary

新系统以 `D:\work\github\postpub` 为唯一主工程根目录。

硬约束：

- 不在 `AIWriteX/` 内新增任何代码或文档
- `AIWriteX/` 仅作为只读参考源
- 新系统落地目录固定为 `postpub/` 根目录
- Web 与 Desktop 共享同一套前端与同一套后端协议

当前实际技术栈：

- Rust workspace
- Axum API
- Vue + TypeScript + Vue Router + Pinia
- Desktop 宿主预留为 Tauri 方向，但尚未完整接入

## Root Structure

```text
postpub/
  Cargo.toml
  docs/
  frontend/
  crates/
    postpub-types/
    postpub-core/
    postpub-api/
  apps/
    desktop/
    web-launcher/
  scripts/
  AIWriteX/   # read-only reference
```

## Layer Responsibilities

### `crates/postpub-types`

放共享结构：

- API 响应结构
- 配置模型
- 模板模型
- 文章模型
- 生成任务模型
- SSE 事件模型

### `crates/postpub-core`

放纯业务逻辑：

- 路径发现与目录初始化
- 配置读写
- 模板 CRUD
- 文章 CRUD
- Markdown/HTML/Text 预览转换
- AIForge 检索与引用抽取
- 文章生成编排

这一层不依赖 Axum，也不依赖前端框架。

### `crates/postpub-api`

放 Axum 服务层：

- `/api/system/*`
- `/api/config/*`
- `/api/templates/*`
- `/api/articles/*`
- `/api/generation/*`
- SSE 任务事件流
- 前端静态资源托管

### `frontend`

共享前端工程：

- 概览页
- 配置页
- 模板页
- 文章页
- 生成页
- 架构页

### `apps/web-launcher`

负责启动 Axum 服务并提供 Web 模式入口。

### `apps/desktop`

当前是桌面宿主占位目录。

目标边界仍然是：

- 启动内嵌 Axum
- 用 Tauri WebView 打开本地地址
- 只承载宿主能力，不复制业务 API

## Runtime Modes

### Web Mode

当前已实现：

- `cargo run -p postpub-web-launcher`
- Axum 提供 API 与静态前端
- 浏览器直接访问本地地址

### Desktop Mode

当前状态：

- 仅保留宿主入口占位
- 尚未完成 Tauri 外壳接入

规划边界：

- Desktop 内嵌 Axum
- WebView 访问本地服务地址
- 宿主层只提供文件对话框、窗口、通知等原生能力

## Shared API Strategy

统一 API 协议：

- `/api/system/health`
- `/api/system/paths`
- `/api/config`
- `/api/config/default`
- `/api/config/ui`
- `/api/templates/...`
- `/api/articles/...`
- `/api/generation/tasks`
- `/api/generation/tasks/{id}`
- `/api/generation/tasks/{id}/events`

统一前端策略：

- 同一套路由
- 同一套 Pinia store
- 同一套 API client

## AIForge In Rust

当前 AIForge 替代实现已经放在 Rust 核心中。

已实现能力：

- 搜索模式：基于 Google News RSS 拉取结果
- 引用模式：直接抓取参考 URL，抽取标题、日期、正文
- 结果标准化：统一映射到共享 `SearchResult`
- 生成模式：Rust 组合逻辑生成 Markdown/HTML/Text 草稿
- 模板注入：支持 `{{title}}`、`{{summary}}`、`{{generated_at}}`、`{{content}}`

当前不包含：

- Python AIForge 桥接
- 外部 LLM 写作链路

## Data Compatibility

继续兼容的文件与目录形态：

- `config.yaml`
- `aiforge.toml`
- `ui_config.json`
- `publish_records.json`
- `templates/<category>/*.html`
- `output/article/*`

## Deferred Items

当前明确暂缓：

- 微信发布链路
- 完整 Tauri 宿主
- 更复杂的模型驱动写作链路

## Validation Status

截至 `2026-03-29` 已验证：

- `cargo test`
- `frontend npm run test`
- `frontend npm run build`
- 浏览器真实验收：配置页、模板页、生成页、文章页
