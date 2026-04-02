# 项目全局规则

## 编码

- 所有包含中文的源码文件、locale 文件、Markdown 文件和配置文件，必须使用 `UTF-8` 编码保存。
- 编辑中文内容时，不要引入乱码，也不要混用 `GBK`、`ANSI` 等其他编码。
- 创建 HTML 文档时，保留 `<meta charset="UTF-8">` 或等效的 UTF-8 声明。

## 前端多语言

- 前端所有面向用户的文案，原则上都必须通过 i18n 管理，不要直接硬编码在页面里。
- 新增或修改 UI 文案时，必须在同一次变更中同步更新 `frontend/src/locales/` 下的语言文件。
- 当前仅支持 `zh-CN` 和 `en-US`，不要再引入其他语言。
- 如需调整语言配置，同时检查：
  - `frontend/src/locales/`
  - `frontend/src/utils/i18n.ts`
  - `frontend/i18n.config.cjs`

## 中文文案

- 中文 locale 内容必须使用自然、可读的简体中文，并以 UTF-8 保存。
- 不要提交乱码、错码或不可读的中文替代文本。

## 自查

每加一个新功能，使用agent-browser技能自查
