# 点屿 5.0 原生运行时

解压完整运行包后启动 `LyricIsland.exe`。托盘的控制中心入口打开 `LyricIsland.Center.exe`，首次进入概览。请保留同目录的 DLL、Assets 和 licenses。

## 程序组成

| 文件 | 职责 |
| --- | --- |
| `LyricIsland.exe`、`dotisle_core.dll` | 灵动岛、后台服务、业务数据和设置持久化 |
| `LyricIsland.Center.exe` | Go + MyGo / Direct3D11 控制中心，统一承载概览、组件、计时、待办、通知、设置和插件 |
| `dotisle_desktop.dll` | 通过 C ABI 嵌入控制中心进程的桌面组件渲染器 |
| 其他 `dotisle_*.dll` | 音乐、歌词、通知、设备、计时等功能模块 |
| `LyricIsland.Updater.exe` | 更新安装、文件校验和失败回滚 |

旧设置源码 `Rust/crates/dotisle-app` 已移除。新构建不再携带 `LyricIsland.Settings.exe` 或 `dotisle_settings.dll`。关闭控制中心窗口会隐藏它；桌面组件继续由同一进程运行，再次打开会唤起该进程。退出点屿会结束它启动的控制中心。

控制中心从主程序的状态管道读取快照，再把快照复制给桌面 DLL。业务修改通过同一原生协议提交。没有桌面组件时不启动组件渲染线程。

## 设置与数据

便携版把设置和缓存保存在程序旁的 `Data`；主程序支持 `--data-dir <目录>`。测试实例使用独立数据目录和 `LyricIsland.Settings.test.*` 管道，避免影响正常使用的数据。

控制中心使用随包 HarmonyOS Sans；歌词字体可在灵动岛设置中选择。字体来源和许可见 [FONT-NOTICE.md](FONT-NOTICE.md)。

## 构建与验证

当前命令见 [docs/BUILDING.md](docs/BUILDING.md)。运行包支持以下无窗口验证入口：

```powershell
./LyricIsland.Center.exe --module-health ./health
./LyricIsland.Center.exe --verify ./verification
```

第一条检查桌面 ABI 与主题模块；第二条用实际生产视图和桌面渲染器生成页面、组件截图及 JSON 报告。运行时集成验收另外检查进程数量、组件增删与缩放、布局保存和重启恢复。

架构及旧版本升级边界见 [统一控制中心说明](docs/UNIFIED-CONTROL-CENTER.md)。3.5.x 旧更新助手仍可能要求旧设置文件，去除旧设置的新包须先验证迁移路径。
