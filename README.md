<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/brand/dotisle-wordmark-light.svg">
  <img src="docs/brand/dotisle-wordmark.svg" width="440" alt="点屿 · DotIsle">
</picture>

# 点屿 · DotIsle

**把音乐、专注和日常提醒，留在桌面上的一座小岛。**

Windows 10 / 11 · x64 · 原生桌面应用

[下载正式版](https://github.com/Heiamer/DotIsle/releases/latest) · [体验 V5.0RC1](https://github.com/Heiamer/DotIsle/releases/tag/v5.0.0-rc1) · [更新日志](CHANGELOG.md) · [反馈问题](https://github.com/Heiamer/DotIsle/issues)

</div>

## 5.0，全新的控制中心

音乐、日历、用机时长、专注与待办，放在同一张概览里。卡片视图方便一眼查看，滚动视图让内容逐页展开；你可以选择组件、调整排列、拖动边角改变大小，也可以增删滚动页。

控制中心采用固定窗口、HarmonyOS Sans、柔和的连续曲线边角与分层阴影。导航平时收起，点击左上按钮展开左侧白色毛玻璃栏，选好页面后自动隐藏。明暗外观、背景和动效有各自的设置；动态取色 **Beta** 默认关闭，开启后使用低饱和的浅彩色。

**V5.0RC1 是预发行测试版本。** 日常使用建议选择正式版；需要体验和反馈 5.0 时，再进入测试版通道。RC1 更新内容与已知限制见 [发行说明](release-notes/DotIsle-5.0.0-rc1-Notes.md)。

## 岛上与桌面上

| 功能 | 可以做什么 |
| --- | --- |
| **音乐与歌词** | 查看歌曲、封面、原文和翻译，播放暂停、切歌与调节进度。概览音乐卡使用磨砂封面背景；音乐岛的音量图标支持悬停提示与滚轮调节。具体控制能力取决于播放器接口。 |
| **专注与计时** | 番茄钟、倒计时、秒表与多阶段流程，支持暂停、恢复及多项目。岛体可显示进行中的计时。 |
| **待办与提醒** | 新建、编辑、完成和撤销待办，设置日期与时间提醒，也可快速记下一件小事。 |
| **设备与通知** | 设备连接、电量不足、验证码和天气提醒进入岛体。微信、飞书、钉钉和企业微信可只提示“有新消息”，不展示消息正文。 |
| **桌面组件** | 时钟、日历、待办、专注、音乐、天气、便签、无线设备电量、日程倒计时、用机时长等，可独立放置和配置。 |
| **插件积木** | 添加自己的本地插件，把文字、数值、进度等内容接到灵动岛、概览和桌面。 |

音乐位于概览卡片中。各项提醒、光效、动态取色与动效按设置启用，界面不会要求你同时使用所有功能。

## 下载与开始使用

从 [Releases](https://github.com/Heiamer/DotIsle/releases) 选择版本。5.0 测试包位于 [V5.0RC1 发行页](https://github.com/Heiamer/DotIsle/releases/tag/v5.0.0-rc1)，由 [Windows Release 工作流](.github/workflows/windows-release.yml) 构建并在验收通过后发布。

| 文件 | 用途 |
| --- | --- |
| `DotIsle-<版本>-Setup.exe` | 安装版，提供安装向导、开始菜单和卸载入口。 |
| `DotIsle-<版本>-Portable.zip` | 便携版，完整解压到可写目录后运行 `LyricIsland.exe`。 |
| `DotIsle-<版本>-Source.zip` | 对应发行版本的源码与资源。 |
| `DotIsle-<版本>-SHA256.txt` | 下载文件的 SHA-256 校验清单。 |
| `DotIsle-<版本>-Validation.zip` | 自动验收报告与界面记录，供排查和开发使用。 |

1. 安装，或把便携包完整解压；不要在压缩包内直接运行 EXE。
2. 启动 `LyricIsland.exe`，从系统托盘打开控制中心。
3. 打开播放器并播放音乐，或直接开始专注、记录待办、选择桌面组件。
4. 进入设置，按需要调整灵动岛、外观、提醒与更新通道。

运行发行包不需要安装 Rust、Go、Python 或 .NET。请保留程序旁的 DLL、资源、字体和更新助手，不要只复制一个 EXE。关闭控制中心后，灵动岛和桌面组件可以继续运行；完全退出点屿后不再提供提醒。

<details>
<summary>为什么同时有 Center.exe 和 Settings.exe？</summary>

5.0 的可见控制中心是 `LyricIsland.Center.exe`。`LyricIsland.Settings.exe` 是仍保留的 Rust 宿主，负责桌面组件，并兼容旧设置入口；主程序通常以隐藏模式启动它。两者目前是迁移期间的分工。直接删除 Settings 会导致桌面组件不可用，普通使用只需启动 `LyricIsland.exe`。

</details>

## 正式版与测试版更新

在 **控制中心 → 设置 → 关于与更新** 中检查更新和切换通道：

- **正式版**：默认通道，接收正式发行版本。
- **测试版**：体验包括 RC 在内的预发行版本，适合愿意反馈问题的用户。

下载和校验完成后按界面提示安装。内部模块更新带摘要校验、替换和失败回滚；旧版本更新器使用的 `LyricIsland-*` 兼容包也会随发行保留。

手动换包前先退出旧版本，备份原数据目录。需要迁移时，将旧版 `Data` 复制到新目录；不要让两个版本同时写同一份数据。测试版配置是否适合退回旧版，需按具体版本确认，建议单独保留备份。

## 写一块自己的插件

从侧栏进入 **插件**，选择包含 `manifest.json` 的本地文件夹，即可连接一个插件。清单决定它出现在灵动岛、概览还是桌面；`state.json` 提供实时内容。安装后可以启用、停用、移除，或把对应卡片加到概览与桌面。

API v1 提供四种基本积木：`text`、`metric`、`progress`、`divider`，支持通过字段路径绑定 JSON 数据。你可以用自己熟悉的语言更新数据文件，点屿定期读取并刷新展示。

- [插件开发文档](docs/PLUGIN-DEVELOPMENT.md)：格式、限制、安装方法与三端展示。
- [Hello 示例](examples/plugins/hello)：清单、示例数据与 Python 数据生产者。
- [模块开发说明](docs/CORE-MODULES.md)：面向内部原生模块开发，与数据插件分开说明。

当前插件是本地数据驱动展示。数据生产者由用户独立运行，点屿不会自动执行插件内的脚本或 DLL。

## 使用范围

- **音乐**：连接和控制能力取决于播放器、版本与媒体接口。网易云相关说明见 [连接与进度文档](NETEASE-SEEKFIX.md)。
- **消息提示**：部分客户端只改变托盘红点，没有任务栏闪烁或系统通知，可能无法触发。四个聊天客户端的所有版本组合尚未逐一实测。
- **设备电量**：需要 Windows 或设备提供可读取的电量数据；读不到时不会虚构百分比。
- **天气**：填写城市并联网后获取，降水开始和结束时间属于预报，受数据覆盖与更新影响。
- **用机时长**：记录点屿运行期间的活跃使用，不补算点屿未运行时的 Windows 历史记录。
- **材质与性能**：云母、模糊和实际流畅度受 Windows 版本、显卡、DPI 与系统合成影响。RC1 不承诺所有设备稳定 120Hz。

## 源码与构建

灵动岛与桌面组件使用 Rust / Win32 / Vello Hybrid / wgpu，新的控制中心使用 Go / MyGo / Direct3D，运行时不依赖 WebView。歌词、音乐、通知、计时、组件等功能通过独立原生模块提供。

推荐使用仓库的 [Windows Release 工作流](.github/workflows/windows-release.yml)。它会核对版本和许可，构建 Rust 程序、服务 DLL 与 Go 控制中心，运行全量回归、界面及模块更新回滚验收，最后生成安装版、便携版、OTA 和源码包。

本地构建请使用 [tools/li-cargo.ps1](tools/li-cargo.ps1)，控制中心依赖与工具链版本以 [go.mod](Go/settings-center/go.mod) 为准。打包入口为 [build/package_release.py](build/package_release.py)，前置检查见 [build/package_preflight.py](build/package_preflight.py)。自动检查范围不等于所有播放器、硬件和真实桌面操作均已验收。

## 许可与反馈

项目采用 [MIT 许可](LICENSE)。第三方与字体声明见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)、[FONT-NOTICE.md](FONT-NOTICE.md) 和 [licenses](licenses/)。发行包保留相关许可原文。

在 [Issues](https://github.com/Heiamer/DotIsle/issues) 反馈时，请附点屿版本、Windows 版本、播放器或设备版本、复现步骤和截图。分享日志前移除个人信息。

<details>
<summary>3.2.6 历史版本下载</summary>

保留原有下载地址，供旧版本用户查找：

[安装版](https://github.com/Heiamer/DotIsle/releases/download/v3.2.6/DotIsle-3.2.6-Setup.exe) · [便携版](https://github.com/Heiamer/DotIsle/releases/download/v3.2.6/DotIsle-3.2.6-Portable.zip) · [源码](https://github.com/Heiamer/DotIsle/releases/download/v3.2.6/DotIsle-3.2.6-Source.zip) · [SHA256](https://github.com/Heiamer/DotIsle/releases/download/v3.2.6/DotIsle-3.2.6-SHA256.txt) · [验证记录](https://github.com/Heiamer/DotIsle/releases/download/v3.2.6/DotIsle-3.2.6-Validation.zip) · [发行说明](https://github.com/Heiamer/DotIsle/releases/tag/v3.2.6)

</details>
