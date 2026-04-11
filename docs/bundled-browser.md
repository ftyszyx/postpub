# 内置浏览器约定

`postpub` 现在支持通过远端配置自动维护内置浏览器，并用这个浏览器来驱动 `agent-browser`。

默认使用的远端配置地址：

```text
https://www.bytefuse.cn/clonerweibo.json
```

当前会读取其中两个字段：

- `chrome_asset_url`
- `chrome_version`

## 默认目录

运行时目录：

```text
<app_root>/runtime
```

内置浏览器目录：

```text
<app_root>/runtime/browser
```

浏览器用户数据目录：

```text
<app_root>/runtime/profiles
```

其中 `<app_root>` 就是 `postpub` 的应用数据根目录。

## 运行流程

每次 `postpub` 需要调用 `agent-browser` 做自动化前，会按下面的顺序处理：

1. 如果设置了 `POSTPUB_BROWSER_EXECUTABLE`，直接使用这个路径
2. 否则请求远端 `clonerweibo.json`
3. 读取本地 `runtime/browser/postpub-browser.json`
4. 如果本地没有已下载浏览器，或者本地记录的 `chrome_version` / `chrome_asset_url` 和远端不一致，就重新下载
5. 下载完成后解压到 `runtime/browser`
6. 调用 `agent-browser --executable-path <下载后的浏览器>`

这意味着默认情况下，不再依赖系统里手工安装的 Chrome 版本。

## 发布平台隔离

现在每个发布平台配置都会使用自己独立的 Chrome profile 目录：

```text
runtime/profiles/<publish_target_id_标准化后>
```

例如：

```text
runtime/profiles/publish-wechat-1
```

这样做有两个好处：

- 不同发布平台之间的 Cookie、Local Storage、扩展状态互相隔离
- 某个平台登录过一次后，下一次继续使用同一个 profile，一般不需要重新登录

浏览器二进制和 profile 目录是分开的，所以即使 Chrome 版本升级重新下载，登录态目录也不会被一起删掉。

## 自动探测的可执行文件

当前会优先探测以下几类常见布局：

- `runtime/browser/chrome.exe`
- `runtime/browser/chromium.exe`
- `runtime/browser/chrome`
- `runtime/browser/chromium`
- `runtime/browser/chrome-win64/chrome.exe`
- `runtime/browser/chrome-win32/chrome.exe`
- `runtime/browser/chrome-linux64/chrome`
- `runtime/browser/chrome-linux/chrome`
- `runtime/browser/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing`
- `runtime/browser/Google Chrome.app/Contents/MacOS/Google Chrome`
- `runtime/browser/Chromium.app/Contents/MacOS/Chromium`

只要下载包里存在其中一种布局，发布流程在调用 `agent-browser` 时就会自动附带 `--executable-path`。

## 覆盖方式

如果需要手动指定浏览器路径，可以设置环境变量：

```text
POSTPUB_BROWSER_EXECUTABLE
```

这个变量的优先级高于远端配置下载逻辑。

## 本地版本记录

下载完成后，会在下面这个文件里记录当前已同步的浏览器版本：

```text
runtime/browser/postpub-browser.json
```

当前记录的主要信息包括：

- `chrome_version`
- `chrome_asset_url`
- `executable_relative_path`
- `synced_at`

## 配置地址覆盖

如果后续不想写死 `www.bytefuse.cn`，也可以通过环境变量覆盖远端配置地址：

```text
POSTPUB_BROWSER_CONFIG_URL
```

## 后续建议

桌面端正式打包时，建议在应用启动阶段就预热这套下载逻辑，避免第一次发布任务时才开始下载浏览器。
