# postpub

`postpub` 是新的主工程根目录。

当前约束：

- 所有新代码、文档、脚本都写在 `D:\work\github\postpub`
- `AIWriteX/` 只作为只读参考源，不再写入任何新实现
- 当前技术栈为 `Rust + Axum + Vue + TypeScript`
- 前端使用 `Vue + Vue Router + Pinia`
- Web 与后续 Desktop 模式共享同一套前端和同一套 API

## 当前实现状态

已实现：

- Cargo workspace 与根目录工程结构
- `postpub-types`、`postpub-core`、`postpub-api`
- `apps/web-launcher`
- Vue 前端主界面与页面路由
- 配置管理
- 模板管理
- 文章管理与预览
- Rust 版 AIForge 检索与引用抓取
- Rust 版文章生成任务与 SSE 事件流
- 前端测试与后端测试

暂缓：

- 微信发布链路
- 完整 Tauri 桌面宿主集成

## 目录结构

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
  AIWriteX/   # 只读参考
```

## 启动方式

Web 模式：

```powershell
cargo run -p postpub-web-launcher
```

启动后访问：

```text
http://127.0.0.1:3000
```

前端构建产物由 Rust 服务直接托管，`apps/web-launcher` 会读取 `frontend/dist`。

前端单独构建：

```powershell
cd frontend
npm install
npm run build
```

## browser agent 编译

```
cargo build -p postpub-agent-browser
```

## Web 调试

前端热更新联调方式：

1. 启动后端服务

```powershell
cargo run -p postpub-web-launcher
```

2. 启动 Vite 开发服务器

```powershell
cd frontend
npm install
npm run dev
```

3. 在浏览器打开

```text
http://127.0.0.1:5173
```

说明：

- Vite 开发服务器会将 `/api` 和 `/images` 代理到 `http://127.0.0.1:3000`
- 默认代理目标可通过环境变量 `POSTPUB_DEV_PROXY_TARGET` 覆盖
- 如果只想验证完整打包后的 Web 形态，继续使用 `cargo run -p postpub-web-launcher` 并访问 `http://127.0.0.1:3000`

## 多语言

前端目前只支持两种语言：

- 中文 `zh-CN`
- 英文 `en-US`

相关语言资源位于：

```text
frontend/src/locales/
```

新增或修改前端界面文案时，需要同步更新对应 locale 文件，并保持中文内容使用 UTF-8 编码保存。

## 测试命令

后端：

```powershell
cargo test
```

前端：

```powershell
cd frontend
npm run test
npm run build
```

## 关键说明

- `config.yaml`、`aiforge.toml`、`ui_config.json` 继续保留兼容路径
- 生成页支持使用本地 demo 参考源完成无外网演示
- 文章文件名现在支持保留中文主题
- `apps/desktop` 目录目前仍是占位宿主，后续再接入真正的 Tauri 外壳
