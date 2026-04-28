# 微信公众号发布流程 API 交互分析

本文记录微信公众号后台 `https://mp.weixin.qq.com/` 中，图文草稿从创建、设置封面到发表的前端 API 交互流程，用于后续固化 `WechatPublisher` 适配器。

> 说明：草稿创建、正文图片上传、正文选封面、保存封面等链路已有实际请求观察；最终发表的最后一步没有点击确认，避免误发布，最终提交接口来自已加载前端 JS 的静态追踪。

## 基础上下文

微信公众号后台主要依赖浏览器登录态和 URL 中的 `token` 参数。自动化入口建议先打开后台首页，确认当前页面可以提取 `token`：

```text
https://mp.weixin.qq.com/
```

常见页面：

```text
# 草稿列表
/cgi-bin/appmsg?begin=0&count=10&type=77&action=list_card&token=<token>&lang=zh_CN

# 新建图文编辑器
/cgi-bin/appmsg?t=media/appmsg_edit_v2&action=edit&isNew=1&type=77&token=<token>&lang=zh_CN

# 已保存草稿编辑器
/cgi-bin/appmsg?t=media/appmsg_edit&action=edit&reprint_confirm=0&timestamp=<timestamp>&type=77&appmsgid=<appmsgid>&token=<token>&lang=zh_CN
```

关键页面信号：

- 标题输入框：`.js_title` 或 `textarea#title`
- 作者输入框：`.js_author` 或 `input#author`
- 正文编辑器：`.ProseMirror[contenteditable="true"]` 或最后一个 `div[contenteditable="true"]`
- 封面区域：`#js_cover_area`
- 发表按钮：`#js_send`

## 创建草稿

打开新建编辑器：

```text
GET /cgi-bin/appmsg?t=media/appmsg_edit_v2&action=edit&isNew=1&type=77&token=<token>&lang=zh_CN
```

填写标题、作者、正文、摘要后，首次保存会走：

```text
POST /cgi-bin/operate_appmsg?t=ajax-response&sub=create&type=77&token=<token>&lang=zh_CN
```

核心表单字段：

```text
count=1
title0=<标题>
author0=<作者>
digest0=<摘要>
content0=<正文 HTML>
need_open_comment0=1
reply_flag0=2
auto_elect_comment0=1
auto_elect_reply0=1
```

典型响应：

```json
{
  "base_resp": { "ret": 0, "err_msg": "" },
  "ret": "0",
  "appMsgId": 100000876,
  "data_seq": "4492886931013566466",
  "msg_index_id_list": ["0_100000876_0"],
  "filter_content_html": [{ "content": "..." }]
}
```

保存成功后，编辑器地址会从 `isNew=1` 变成带 `appmsgid` 的编辑地址。后续保存改用更新接口：

```text
POST /cgi-bin/operate_appmsg?t=ajax-response&sub=update&type=77&token=<token>&lang=zh_CN
```

更新草稿时额外带：

```text
AppMsgId=<appmsgid>
data_seq=<上次保存返回的 data_seq>
operate_from=Chrome
isnew=0
articlenum=1
```

保存草稿前后常见附加请求：

```text
POST /cgi-bin/masssend?action=check_music
POST /cgi-bin/operate_appmsg?sub=collaboration_edit&appmsgid=<appmsgid>
POST /cgi-bin/appmsg?action=get_appmsg_update_history&appmsgid=<appmsgid>&offset=0&limit=8
```

## 正文图片上传

如果封面策略是“先上传到正文，再从正文选择”，图片上传请求为：

```text
POST /cgi-bin/filetransfer?action=upload_material&f=json&scene=8&writetype=doublewrite&groupid=1&ticket_id=<ticket_id>&ticket=<ticket>&svr_time=<svr_time>&token=<token>&lang=zh_CN&seq=<seq>&t=<random>
```

典型响应：

```json
{
  "base_resp": { "ret": 0, "err_msg": "ok" },
  "cdn_url": "https://mmbiz.qpic.cn/mmbiz_png/.../0?wx_fmt=png&from=appmsg",
  "content": "100000875",
  "location": "bizfile",
  "type": "image",
  "ai_status": 1
}
```

上传后前端会更新最近使用素材：

```text
POST /cgi-bin/modifyfile?oper=updaterecent&fileid=<fileid>
```

## 封面：从正文选择

页面交互入口：

- 封面区域：`#js_cover_area`
- 封面按钮层：`.js_cover_btn_area`
- 从正文选择：`a.js_selectCoverFromContent`

交互流程：

1. 点击封面区域。
2. 选择“从正文选择”。
3. 在“选择图片”弹窗中选择可见的正文图片，如 `.appmsg_content_img.cover`。
4. 点击“下一步”。
5. 在“编辑封面”弹窗中确认 `2.35:1` 和 `1:1` 裁剪。
6. 点击“确认”。

封面预览成功信号：

```js
Array.from(document.querySelectorAll("#js_cover_area *")).some((el) =>
  getComputedStyle(el).backgroundImage.includes("mmbiz")
)
```

## 封面裁剪接口

编辑封面弹窗会先尝试获取智能裁剪建议：

```text
POST /cgi-bin/cropimage?action=crop_suggestion
```

请求体：

```json
{
  "data": "{\"url\":\"<图片 URL>\",\"ratio_type\":[4,2]}"
}
```

比例映射：

```text
16_9   -> 1
1_1    -> 2
3_4    -> 3
2.35_1 -> 4
```

最终裁剪 CDN 调用是：

```text
POST /cgi-bin/cropimage?action=crop_multi
```

表单字段：

```text
imgurl=<原图 URL>
size_count=2
size0_x1=<百分比 x1>
size0_y1=<百分比 y1>
size0_x2=<百分比 x2>
size0_y2=<百分比 y2>
size1_x1=<百分比 x1>
size1_y1=<百分比 y1>
size1_x2=<百分比 x2>
size1_y2=<百分比 y2>
```

普通首图文通常裁两份：

```text
2.35_1 消息列表封面
1_1    转发卡片和公众号主页封面
```

`crop_multi` 响应的 `result[]` 会包含：

```json
[
  {
    "cdnurl": "https://mmbiz.qpic.cn/mmbiz_jpg/...",
    "file_id": 100000877,
    "width": 900,
    "height": 383
  }
]
```

如果图片不是微信 CDN 图片，前端可能先转存：

```text
POST /cgi-bin/uploadimg2cdn?t=ajax-editor-upload-img
```

请求体：

```text
imgUrl=<外部图片 URL>
ai_watermark=<0 或 1>
```

## 封面保存字段

裁剪完成后，仍然通过草稿更新接口保存：

```text
POST /cgi-bin/operate_appmsg?t=ajax-response&sub=update&type=77&token=<token>&lang=zh_CN
```

封面相关字段：

```text
cdn_url0=<主封面 URL>
cdn_235_1_url0=<2.35:1 裁剪 URL>
cdn_1_1_url0=<1:1 裁剪 URL>
cdn_3_4_url0=
cdn_16_9_url0=
cdn_url_back0=<原始图片 URL>
crop_list0=<裁剪信息 JSON>
fileid0=<素材 fileid，正文选封面时可能为空>
last_choose_cover_from0=0
```

`crop_list0` 示例：

```json
{
  "crop_list": [
    {
      "ratio": "2.35_1",
      "x1": 0,
      "y1": 0,
      "x2": 0,
      "y2": 0,
      "file_id": 100000877
    },
    {
      "ratio": "1_1",
      "x1": 0,
      "y1": 0,
      "x2": 0,
      "y2": 0,
      "file_id": 100000878
    }
  ],
  "crop_list_percent": [
    {
      "ratio": "2.35_1",
      "x1": 0,
      "y1": 0,
      "x2": 1,
      "y2": 1
    }
  ]
}
```

注意：前端静态代码里有一处类似 `["content, lib"].includes(sourceMark)` 的判断，实际不会匹配 `"content"` 或 `"lib"`。实测正文选封面保存时 `last_choose_cover_from0=0`，适配器应以实际请求为准。

## 封面：AI 生成

封面 AI 入口：

```js
showAiImageDialog({
  entry: "cover-setter",
  insert(images) {
    const selected = images[0];
  }
})
```

AI 生图接口统一使用：

```text
/cgi-bin/mpaigenpic
```

常见初始化请求：

```text
GET /cgi-bin/mpaigenpic?action=get_session
GET /cgi-bin/mpaigenpic?action=get_example
GET /cgi-bin/mpaigenpic?action=get_style
GET /cgi-bin/mpaigenpic?action=get_biz_recent_img_list
GET /cgi-bin/mpaigenpic?action=process_terms_of_use
```

开始生成：

```text
POST /cgi-bin/mpaigenpic?action=start_ai_creation
```

body 是 `data=<JSON>`：

```json
{
  "prompt": "<提示词>",
  "examples": [],
  "revise": false,
  "session_id": "<session_id>",
  "gen_scene": 1,
  "scale": "<选中的比例值>",
  "prompt_type": 2
}
```

响应返回：

```json
{
  "task_id": "<task_id>",
  "is_sensitive_prompt": false
}
```

轮询图片：

```text
GET /cgi-bin/mpaigenpic?action=get_ai_pic
```

请求参数：

```text
task_id=<task_id>
session_id=<session_id>
```

图片状态：

```text
2 加载中
3 成功
4 失败
```

选中 AI 图片后插入素材：

```text
POST /cgi-bin/mpaigenpic?action=insert_ai_pic
```

body：

```json
{
  "pic_id": "<图片 id>",
  "prompt": "<提示词>",
  "task_id": "<task_id>",
  "origin_pic_id": "<原始图片 id>",
  "session_id": "<session_id>"
}
```

响应：

```json
{
  "fileid": "<fileid>",
  "cdn_url": "https://mmbiz.qpic.cn/..."
}
```

随后 AI 图片会进入同一套封面编辑流程：

```text
AI 生成图片 -> insert_ai_pic -> 编辑封面 -> crop_multi -> operate_appmsg sub=update
```

## 点击发表前置流程

点击编辑页 `#js_send` 后，前端可能先触发创作来源强制声明检查：

```text
POST /cgi-bin/setclaimsourcetype?action=forceclaimsource
```

请求体：

```json
{
  "data": "{\"appmsgid\":\"100000876\",\"item_show_type_list\":[0]}"
}
```

如果响应包含：

```json
{
  "base_resp": { "ret": 0, "err_msg": "ok" },
  "force_claim_source": 1
}
```

页面会提示内容可能涉及时事、公共政策、社会事件或 AI 生成，需要声明创作来源。用户可选择“无需声明并发表”或“去声明”。

继续后会打开发表弹窗，并请求发表页数据：

```text
GET /cgi-bin/masssendpage?f=json&preview_appmsgid=<appmsgid>&token=<token>&lang=zh_CN&ajax=1&fingerprint=<fingerprint>&random=<random>
```

典型响应字段：

```json
{
  "base_resp": { "ret": 0, "err_msg": "ok" },
  "mass_send_left": 1,
  "operation_seq": "1777389891_n7HQalHgWKtawgR0",
  "strategy_info": "{\"wx_protect\":1,\"mobile\":\"+86186******26\",\"protect_status\":3}",
  "contact_group_list": "{\"group_info_list\":[]}"
}
```

发表弹窗包含：

- 发表
- 群发通知
- 分组通知
- 定时发表
- 取消

## 最终发表提交

最终按钮会构造 `postData`：

```json
{
  "ack": "",
  "code": "",
  "reprint_info": "",
  "reprint_confirm": 0,
  "list": "",
  "groupid": "",
  "sex": "0",
  "country": "",
  "province": "",
  "city": "",
  "send_time": 0,
  "type": 10,
  "share_page": 1,
  "synctxweibo": 0,
  "operation_seq": "<masssendpage 返回值>",
  "scene_replace": "<cgiData.scene_replace>",
  "req_id": "<32 位随机串>",
  "req_time": "<当前毫秒时间 + diff>",
  "sync_version": 1,
  "isFreePublish": true,
  "appmsgid": "<appmsgid>",
  "isMulti": false
}
```

分组字段：

```text
groupid=-1           # 全部
groupid=<group_id>   # 指定分组
groupid=0            # 卡券分组
card_tag_id=<id>     # 卡券分组 id
```

地区和性别筛选：

```text
country=<国家或空>
province=<省>
city=<市>
sex=<0/1/2>
```

定时发表：

```text
send_time=<Unix 时间戳>
```

验证码：

```text
imgcode=<验证码>
```

最终提交前检查链路：

```text
POST /cgi-bin/masssend?t=ajax-response&for_check=1&is_release_publish_page=1
POST /cgi-bin/masssend?action=check_same_material
POST /cgi-bin/masssend?action=get_appmsg_copyright_stat
POST /cgi-bin/masssend?action=check_ad
POST /cgi-bin/masssend?action=check_music
```

安全验证参数来自 `strategy_info`。如果需要扫码或人脸验证，前端会把安全检查参数设为：

```json
{
  "source": "msgs",
  "msgid": "<operation_seq>",
  "distinguish": true
}
```

最终发表接口：

```text
POST /cgi-bin/masssend?t=ajax-response&is_release_publish_page=1
```

定时发表接口：

```text
POST /cgi-bin/masssend?action=time_send&t=ajax-response
```

非自由发布页的老群发接口：

```text
POST /cgi-bin/masssend?t=ajax-response
```

成功响应 `base_resp.ret=0` 后，页面提示“已发表，正在返回首页”，并跳转：

```text
/cgi-bin/home?t=home/index
```

## 适配器实现建议

建议把 `WechatPublisher` 拆成固定步骤，不在生产发布时依赖 AI 临场决策：

```text
ensure_login_token
open_editor
fill_article_fields
upload_body_images
create_or_update_draft
set_cover_from_body
set_cover_from_ai
apply_publish_settings
save_draft
prepublish_check
publish_draft
```

第一阶段建议只稳定实现草稿保存：

```text
创建草稿 -> 填写内容 -> 设置封面 -> 保存草稿
```

最终发表建议作为显式模式开启，并在代码里暴露这些失败分支，而不是过度 fallback：

- 未登录或 token 失效
- 封面裁剪失败
- 创作来源声明必填
- 原创/版权检查失败
- 音频检查失败
- 广告检查失败
- 重复素材确认
- 安全扫码或人脸验证
- 发表额度不足
- 定时发表额度或时间冲突

生产实现中应使用内置浏览器和固定选择器/API 流程；验证选择器和等待条件时可继续用 `agent-browser` 或 `opencli` 分析，但稳定后应固化在适配器内。
