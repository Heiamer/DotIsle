# 第三方组件与许可

本文件对应点屿 3.2.5 歌词与进度实现。项目自有源码的 MIT 许可见根目录 LICENSE，Copyright (c) 2026 Heiamer；以下依赖分别遵循各自许可。其他锁定依赖见 Rust/Cargo.lock。

## 1. 当前歌词实现

歌曲匹配、单时间轴模型、解析、缓存、本地歌词检索与在线适配器采用重新实现的代码。网易云自动进度独立实现只读进程内存读取，通过客户端自身类型信息定位音频对象，并使用受已验证客户端 SHA256 约束的私有字段偏移、指令位置偏移及机器码字节校验；本机页面通道为可选回退。替换范围、兼容差异及验证边界见 REWRITE.md 和 NETEASE-SEEKFIX.md。

## 2. QQ 逐字歌词块解码器

来源：https://github.com/SuJiKiNen/LyricDecoder ，提交 36fc1eaf31f8ef14a23de6f9f1427ce04178a900。

采用 QQMusicDES/des.c 和 des.h，MIT，Copyright (c) 2019 SuJiKiNen。原文件头的 Brad Conte、wangqr 声明保留。DotIsle 将两个左移表达式转换为无符号 WORD，以避免有符号溢出；其余接入由 Rust 实现。源码与修改说明见 Rust/vendor/qq-lyric-codec，完整许可见该目录 LICENSE 和 licenses/qq-lyric-codec-MIT.txt。

## 3. 网易云 EAPI 协议参考

来源：https://github.com/NeteaseCloudMusicApiEnhanced/api-enhanced ，提交 4a7d557326d16d2f1e5172ee69c9471eb3a50173。

MIT，Copyright (c) 2013-2022 Binaryify。providers.rs 的 EAPI 请求封装以其协议为独立替换参考；完整许可见 licenses/NeteaseCloudMusicApi-MIT.txt。没有捆绑其 Node 服务端。

## 4. Rust / Vello 界面栈

界面使用 Vello Hybrid 0.2、wgpu 29 与 Parley 0.5，不使用 GPUI。依赖的锁定版本见 Rust/Cargo.lock，所选许可全文汇总于 licenses/Rust-Dependencies-Licenses.txt。

## 5. 通知监听与旧版 SQLite 数据库兼容代码（随包捆绑 e_sqlite3.dll）

通知监听（只读 Windows 通知数据库 wpndatabase.db）与旧 database/song_maps.db 的可选只读导入共用随包捆绑的 `e_sqlite3.dll`（win-x64，来自 SQLitePCLRaw 的原生 SQLite 构建，以 Apache License 2.0 分发；SQLite 本身处于公有领域：https://www.sqlite.org/copyright.html ）。该 DLL 只以只读方式打开用户数据库，绝不写入或上传内容。SQLitePCLRaw 许可详见 licenses/e_sqlite3-APACHE-2.0.txt 与 https://github.com/ericsink/SQLitePCL.raw 。

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
