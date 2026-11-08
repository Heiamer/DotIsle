# 点屿 3.1.7 本地修复与验证

构建工具与打包入口沿用 BUILD-3.1.4.md。使用 tools/li-cargo.ps1，设置 DOTISLE_ZIG、DOTISLE_DLLTOOL 和可写的 DOTISLE_TARGET_DIR。

1. `test --workspace --offline`：477 项测试，包含独立参考实现生成的 QRC 固定向量、自启动策略和歌曲身份封面缓存回归。
2. `build --release --workspace --bins --examples --offline`。
3. examples/verify_smtc：参数为报告目录、两张不同的本地 PNG；创建独立隐藏 SMTC 会话，无真实播放器控制。
4. examples/verify_lyrics：参数为报告 JSON 路径；只读验证六个歌词来源，记录命中、分数、耗时及是否提供时长。
5. 主程序 --glass-preview、--immersive-preview、--notification-preview，控制中心 --verify 检查正式渲染与生产曲线编辑器。
6. build/package_release.py 使用版本 3.1.7；隔离安装器 AppId 为 DotIsle.QA.Release317。

在线服务协议的核对来源：
- https://github.com/lyswhut/lx-music-desktop/tree/master/src/renderer/utils/musicSdk
- https://github.com/ddddxxx/LyricsKit/blob/master/Sources/LyricsService/Provider/Kugou.swift
- 本段仅记录 3.1.7 的历史构建；当前歌词协议和解码器已经替换，参见 REWRITE.md 与 THIRD-PARTY-NOTICES.md。

未更改 README 下载链接和更新清单；本轮未发布。
