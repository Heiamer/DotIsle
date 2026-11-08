# 点屿 Rust 工作区

当前工作区由九个包组成：

- `lyric-core`：歌词模型、匹配、解析、缓存、本地检索和同步。
- `lyric-backend`：系统媒体、在线歌词、网易云页面进度、存储与更新检查。
- `dotisle-domain`：设置、计时、待办、通知和界面共享状态。
- `dotisle-scene`：场景与布局数据。
- `dotisle-render`：Vello Hybrid / wgpu 原生渲染与文字排版。
- `dotisle-app`：控制中心和设置界面，产物为 `lyric-island-settings.exe`。
- `lyric-native`：灵动岛、通知、动画、音乐控制和主程序。
- `lyric-archive`：更新压缩包与 SHA256。
- `lyric-bootstrap`：安装、卸载、更新与启动器。

在仓库根目录使用 `tools/li-cargo.ps1` 构建和测试；它负责 GNU 工具链、静态 CRT、Zig、LLVM 与链接路径。完整命令和 GitHub Actions 发布流程见 [3.2.4 构建说明](../BUILD-3.2.4.md)。

锁定依赖见 Cargo.lock，许可证见根目录 LICENSE 与 THIRD-PARTY-NOTICES.md。源码中不包含用户数据和编译缓存。
