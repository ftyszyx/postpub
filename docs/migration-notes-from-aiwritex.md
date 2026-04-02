# Migration Notes From AIWriteX

## Purpose

这份文档只记录“参考来源与行为映射”，不复制旧项目实现代码。

## Reference Source

旧项目位置：

- `D:\work\github\postpub\AIWriteX`

角色：

- 只读参考源
- 不再作为新工程写入位置

## Mapping

### UI

- 旧界面结构与导航：参考为新的 Vue 页面信息架构
- 旧配置页：映射到 `frontend` 中的 Config 页面
- 旧模板页：映射到 Templates 页面
- 旧文章页：映射到 Articles 页面
- 旧生成页：映射到 Generation 页面

### Backend

- FastAPI 风格接口：映射到 Axum API
- 旧生成任务链路：映射到 Rust 任务管理与 SSE
- 旧文件读写逻辑：映射到 `postpub-core`

### Data

保持兼容的主要文件形态：

- `config.yaml`
- `aiforge.toml`
- `ui_config.json`
- `publish_records.json`
- 模板目录结构
- 文章输出目录结构

## Boundary

允许参考：

- UI 结构
- 配置格式
- 数据形态
- 业务行为语义

不做的事：

- 不在 `AIWriteX/` 中新增实现
- 不把 `AIWriteX/` 当作新工程落地目录
- 不直接复制受限实现代码作为新实现
