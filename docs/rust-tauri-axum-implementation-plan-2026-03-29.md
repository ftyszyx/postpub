# Postpub Root Rebuild Implementation Plan

日期：`2026-03-29`

这份文档保留“实施计划”文件名，但内容已经按当天的实际落地状态更新。

## Goal

在 `postpub/` 根目录内建立新的主工程，不再在 `AIWriteX/` 中继续演进。

## Phase Status

### Phase 1: Workspace Foundation

状态：已完成

完成内容：

- Cargo workspace 初始化
- `docs / frontend / crates / apps / scripts` 建立
- 根目录约束明确为新主工程入口

### Phase 2: Rust Base Crates

状态：已完成

完成内容：

- `postpub-types`
- `postpub-core`
- `postpub-api`
- `apps/web-launcher`

结果：

- `cargo test` 可通过
- Web 启动器可运行

### Phase 3: Frontend Foundation

状态：已完成

完成内容：

- Vue + TypeScript 工程
- Vue Router
- Pinia
- API client
- Host bridge
- 主布局与导航

### Phase 4: Non-AI Business Modules

状态：已完成

完成内容：

- 配置管理
- 模板管理
- 文章管理
- 文章预览
- 工作目录初始化与兼容文件落盘

### Phase 5: AIForge And Generation

状态：已完成当前 Rust 版本

完成内容：

- Rust 版 AIForge 检索
- 参考 URL 内容抽取
- 统一来源结构
- 生成任务管理
- SSE 事件流
- HTML/Markdown/Text 输出
- 文章自动保存

说明：

- 当前是 Rust 原生实现
- 未接入 Python AIForge
- 未接入外部大模型写作链路

### Phase 6: Publish Chain

状态：部分完成

完成内容：

- 发布记录文件准备

暂缓内容：

- 微信发布链路

### Phase 7: Desktop Host

状态：未完成

当前情况：

- `apps/desktop` 仅保留占位入口

后续目标：

- 接入 Tauri
- 启动内嵌 Axum
- WebView 打开本地服务地址

## Current Acceptance Result

截至 `2026-03-29` 已通过：

1. Rust 后端测试
2. 前端 Vitest 测试
3. 前端生产构建
4. 浏览器端真实交互验收

已验证页面：

- Overview
- Config
- Templates
- Generation
- Articles

## Next Steps

下一阶段建议按下面顺序继续：

1. 完整接入 Tauri 桌面宿主
2. 增加更细的前端页面测试
3. 增加发布链路抽象层
4. 后续再接入新的微信发布方案
