# 第三方组件与许可

本文件对应点屿 5.2.9 随附组件。项目自有源码的 MIT 许可见根目录 LICENSE，Copyright (c) 2026 Heiamer；以下依赖分别遵循各自许可。其他锁定依赖见 Rust/Cargo.lock。

## 1. 当前歌词实现

在线歌词获取顺序、匹配与搜索词变体、在线歌词解析及翻译/音译附着移植自 H0308/taskbar-lyrics-displayer（Apache-2.0，提交 3fae9287dfb0636fc19a0df2587a1a59a54cf9bb），使用 Go/Rust 实现。许可全文与修改声明见 licenses/Lyrics-Apache-2.0.txt 和 licenses/Lyrics-Upstream-NOTICE.txt。LRCLIB lyricsfile 解析使用 go.yaml.in/yaml/v3 v3.0.5，原许可见 licenses/Go/go.yaml.in-yaml-v3.txt。单时间轴模型、缓存和本地歌词检索保留点屿实现。网易云自动进度独立实现只读进程内存读取，通过客户端自身类型信息、指令解码和参数流向自动推导 x86/x64 音频对象、字段和进度缓存，不按版本号或文件哈希设白名单；本机页面通道为可选回退。替换范围、兼容差异及验证边界见 REWRITE.md 和 NETEASE-SEEKFIX.md。

## 2. QQ 逐字歌词块解码器

来源：https://github.com/SuJiKiNen/LyricDecoder ，提交 36fc1eaf31f8ef14a23de6f9f1427ce04178a900。

采用 QQMusicDES/des.c 和 des.h，MIT，Copyright (c) 2019 SuJiKiNen。原文件头的 Brad Conte、wangqr 声明保留。DotIsle 将两个左移表达式转换为无符号 WORD，以避免有符号溢出；其余接入由 Rust 实现。源码与修改说明见 Rust/vendor/qq-lyric-codec，完整许可见该目录 LICENSE 和 licenses/qq-lyric-codec-MIT.txt。

## 4. Rust / Vello 界面栈

界面使用 Vello Hybrid 0.2、wgpu 29 与 Parley 0.5，不使用 GPUI。依赖的锁定版本见 Rust/Cargo.lock，所选许可全文汇总于 licenses/Rust-Dependencies-Licenses.txt。

## 5. 通知监听与旧版 SQLite 数据库兼容代码（随包捆绑 e_sqlite3.dll）

通知监听（只读 Windows 通知数据库 wpndatabase.db）与旧 database/song_maps.db 的可选只读导入共用随包捆绑的 `e_sqlite3.dll`（win-x64，来自 SQLitePCLRaw 的原生 SQLite 构建，以 Apache License 2.0 分发；SQLite 本身处于公有领域：https://www.sqlite.org/copyright.html ）。该 DLL 只以只读方式打开用户数据库，绝不写入或上传内容。SQLitePCLRaw 许可详见 licenses/e_sqlite3-APACHE-2.0.txt 与 https://github.com/ericsink/SQLitePCL.raw 。

## DesktopToast（飞书、钉钉通知）

来源：https://github.com/PepsiColaya/desktop-toast ，提交 `467b66ab6a31aa6db26e86b97f638512eb4e5604`。MIT，Copyright (c) 2026 DesktopToast contributors。完整许可见 `licenses/DesktopToast-MIT.txt`。将 `watcher.py` 中的 Windows `UserNotificationListener` 读取、Toast 文本提取、启动基线和短期去重移植为 Rust 原生模块，使用点屿现有的应用筛选和呈现；不引入 Python、PyQt、权限弹窗或额外常驻进程。

## 6. 字体

西文与数字使用 Google Sans Flex，中文使用 Noto Sans SC，日文使用 Noto Sans JP；均采用 SIL Open Font License 1.1。

Google Sans Flex：Copyright 2025 Google LLC；Noto 字体：Copyright 2014-2021 Adobe (http://www.adobe.com/), with Reserved Font Name 'Source'.

- Google Sans Flex 来源：https://fonts.google.com/specimen/Google+Sans+Flex （2025 年 11 月起以 SIL OFL 发布）
- Noto Sans SC 来源：https://github.com/google/fonts/tree/main/ofl/notosanssc
- Noto Sans JP 来源：https://github.com/google/fonts/tree/main/ofl/notosansjp
- 完整许可：`licenses/GoogleSansFlex-OFL.txt`、`licenses/NotoSansSC-OFL.txt`、`licenses/NotoSansJP-OFL.txt`
- 资源位置：`Assets/Fonts/GoogleSansFlex-*.ttf`、`Assets/Fonts/NotoSansSC.ttf`、`Assets/Fonts/NotoSansJP.ttf`

具体使用说明见 `FONT-NOTICE.md`。

## roxmltree（歌词与通知 XML 解析）

https://github.com/RazrFalcon/roxmltree · 0.20.0 · MIT OR Apache-2.0。采用 MIT 许可副本，见 licenses/roxmltree-MIT.txt。

## material-colors（动态色彩与封面取色）

https://github.com/Aiving/material-colors · 0.4.2 · MIT OR Apache-2.0。它是 Google Material Color Utilities（https://github.com/material-foundation/material-color-utilities，Apache-2.0）的社区 Rust 移植，不是 Google 官方实现。源码随附于 `Rust/vendor/material-colors-0.4.2`，采用 MIT 许可副本，见 licenses/material-colors-MIT.txt。

## Inno Setup（安装向导）

安装器使用 Inno Setup 编译，许可见 licenses/Inno-Setup-License.txt。

## Material Symbols Rounded

Settings and device-status icons use official Material Symbols Rounded by Google, distributed under Apache License 2.0. Source: https://github.com/google/material-design-icons . The exact revision is recorded in Assets/MaterialSymbols/REVISION.txt. The license is retained in licenses/MaterialSymbols-Apache-2.0.txt. Music transport glyphs use the same icon set; the state-dependent button shape is application geometry.

## 本地混合渲染补丁

本轮修改 Vello Hybrid 0.2.0 的数据纹理布局，以及 wgpu-hal 29.0.4 的 DX12 字节码缓存。完整修改源码和原许可随源码包的 `Rust/vendor/` 提供，原项目许可均保留。便携包附带的 Hybrid29 DXBC 由这些源码生成。旧 compute 版本的 Vello/wgpu26 本地补丁不参与本混合版构建。

新增依赖包括 Vello Hybrid/vello_common/vello_sparse_shaders 0.2.0、glifo 0.3.0、Peniko 0.6.1、Kurbo 0.13.1、wgpu 29.0.3 和 core/hal/types 29.0.4。补充许可见 `licenses/Hybrid-Rust-Licenses.txt`，实际锁定依赖见 `Rust/Cargo.lock`。

## AndroidX Material 3 Motion

本版弹簧参数使用 AndroidX Material 3 的 ExpressiveMotionTokens / StandardMotionTokens v0_14_0。Copyright 2024 The Android Open Source Project，Apache License 2.0。求解器为本项目原生 Rust 实现。原始参数表及许可见 licenses/AndroidX-Motion/。

https://github.com/androidx/androidx/tree/androidx-main/compose/material3/material3/src/commonMain/kotlin/androidx/compose/material3/tokens

## 播放主题图标库

播放主题的运输控制图标来自以下开源图标库；礁石主题继续使用项目内置的 Material Symbols Rounded。各库许可如下：

- Material Symbols Rounded（Apache-2.0，Google）https://github.com/google/material-design-icons
- Tabler Icons（MIT）https://github.com/tabler/tabler-icons
- Phosphor Icons（MIT）https://github.com/phosphor-icons/core
- Lucide（ISC）https://github.com/lucide-icons/lucide
- Bootstrap Icons（MIT）https://github.com/twbs/icons
- Remix Icon（Apache-2.0）https://github.com/Remix-Design/RemixIcon
- Feather（MIT）https://github.com/feathericons/feather
- Heroicons（MIT）https://github.com/tailwindlabs/heroicons

逐主题对照与版本见 `Assets/TransportIcons/NOTICE.md`。SVG 保留原始许可注释。


## 重写构建依赖

本轮锁定版本变化涉及 cc、find-msvc-tools、libc、portable-atomic、slab、toml_edit 和 winnow；所选 MIT 许可全文见 licenses/Rewrite-Rust-Licenses.txt。

## iced-x86 与 lazy_static（客户端结构推导）

iced-x86 1.22.0，https://github.com/icedland/iced ，MIT，Copyright (C) 2018-present iced project and contributors。使用其指令解码与信息接口分析本机客户端，完整许可见 licenses/iced-x86-MIT.txt。

lazy_static 1.5.1，https://github.com/rust-lang-nursery/lazy-static.rs ，MIT OR Apache-2.0，为 iced-x86 的间接依赖；采用 MIT 许可副本，见 licenses/lazy_static-MIT.txt。

## 天气数据

天气组件在用户配置城市后请求公开天气数据，并缓存结果；以下为实际数据提供方署名，服务条款与代码许可分别适用。

- [Open-Meteo](https://open-meteo.com/)：天气与地理编码；天气数据 CC BY 4.0。免费接口用于非商业用途并受其调用限额约束。数据经过时段归一化，不表示分钟级雷达临近预报。
- [MET Norway](https://api.met.no/)：Locationforecast，Norwegian Meteorological Institute；CC BY 4.0。使用项目 User-Agent、四位小数坐标及缓存。降水量按 next_1_hours 时段归一化。
- [DWD / Bright Sky](https://brightsky.dev/)：Deutscher Wetterdienst (DWD) open data，经 Bright Sky 提供；DWD 数据 CC BY 4.0，Bright Sky 程序 MIT。选择距请求位置不超过 50 km 的站点，小时累积量归一化处理。
- [wttr.in](https://github.com/chubin/wttr.in)：公共天气服务，程序 Apache-2.0，数据按该服务和上游数据条款使用；三小时预报保持近似标记。
- [7Timer!](https://www.7timer.info/doc.php?lang=en)：全球三小时模型样本；官方允许非商业用途免费使用与再分发，开发者接入通知要求以官方说明为准。仅作为低优先级粗粒度后备。
- [QQ 天气](https://tianqi.qq.com/) 与 [今日头条天气](https://www.toutiao.com/)：公开网页使用的天气接口。保留来源名称，未将其声明为开源数据或无限制商业 API；程序不使用第三方应用私钥或示例密钥。类别预报缺少的雨量和概率保持未知。

部分适配器共享上游气象模型；不同接口数量不代表独立气象观测数量。

## 5.0 Go 原生控制中心

使用 MyGo v0.9.0、purego、go-text/typesetting 与 golang.org/x 库，锁定版本见 Go/settings-center/go.mod；许可全文保存在 licenses/Go。中文字体使用 Noto Sans SC，许可见 licenses/NotoSansSC-OFL.txt。


## BrimDeck 额度采集（Apache-2.0）

`MoFeng2223/BrimDeck`，固定提交 `d3215210e2f337c93784e862bea9830754030819`。将 Codex / Claude 本机记录读取、额度来源目录、DeepSeek / DeepSeek Harness、ZCode、GLM、OpenRouter、New API、Sub2API、Command Code、OpenCode、Kimi、MiMo、Grok Bot、CodeAgent、Cursor、Qoder、千问办公、TRAE、WorkBuddy、千问、豆包／Dola 和火山方舟／BytePlus 的相关逻辑原生转译到 `Rust/crates/lyric-native/src/ai_quota*.rs`；受限自定义脚本兼容上游 `fetchUsage(ctx)` 协议。显式授权入口位于 `Go/settings-center/ui/quota_browser_login.go` 与 `quota_cookie_login.go`；各来源的当前范围见 `docs/AI-QUOTA-NATIVE.md`。修改包括 Windows DPAPI 凭据绑定、限制读取与 HTTPS 目的地、不转发重定向、剩余额度分类和灵动岛提醒。不分发 BrimDeck 二进制文件、.NET 运行时或 Antigravity 适配器。全文、原文件和修改声明：`licenses/BrimDeck-Apache-2.0.txt`、`licenses/BrimDeck-NOTICE.txt`。移植文件按 Apache-2.0 提供，其他点屿自有代码仍按其 MIT 许可。Rust 时间解析用 Chrono 0.4.45，完整上游许可见 `licenses/chrono-0.4.45-LICENSE.txt`。

## 邮箱：功能参考与实际依赖

邮箱交互参考 `flyl1u/fly-island` 的公开 IMAP 使用指南（提交 `569daee3afe3e0041a2362f657f04f5073e85993`，上游仓库 GPL-3.0）。点屿的 Go 邮箱为独立实现，不包含、不链接也不分发 fly-island 的代码、二进制或素材。实际引入的依赖为 `emersion/go-imap/v2` v2.0.0-beta.8、`emersion/go-message` v0.18.2 及 `emersion/go-sasl` v0.0.0-20241020182733-b788ff22d5a6，均为 MIT；全文见 `licenses/Go/emersion-go-{imap,message,sasl}-MIT.txt`。纯文本化使用项目原有的 `golang.org/x/net`（许可见 `licenses/Go/golang.org-x-net.txt`）。

钉钉、飞书使用点屿既有 Windows 通知与 Shell Hook 机制，不引入第三方客户端源码；天气定位调用项目已用的 wttr.in 服务，不打包其源码。

## QuotaView Windows 快捷问答（历史 MIT 版本）

`Duoasa/QuotaView` 的 `v0.8.3-win.13`（提交 `7c163500afb7a75ce6fe48e54664abe83d038870`）。将 `Windows/windows/src/core/codex-desktop.cjs`、`codex-desktop-state.cjs` 中跟随会话和结构化问答的部分逻辑，以及 `Windows/webview2/native/CodexDesktopPipe.cs` 的连接校验移植到 Rust `agent_desktop.rs`。保留上游署名和 MIT 许可全文（`licenses/QuotaView-Windows-MIT.txt`）。圆环 UI 和图标由点屿原生绘制；没有引入 QuotaView 资源、运行时或现行 GPL-3.0-only 分支的实现。BrimDeck 部分仍按原 Apache-2.0 条目处理。
