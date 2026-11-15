# 5.0 统一控制中心打包链

更新于 2026-11-15。当前包只使用 Go 控制中心，桌面组件由 `dotisle_desktop.dll` 嵌入同一进程。原先要求旧设置 EXE 和 DLL 的输入清单已作废。

## 输入与门禁

- Rust：`LyricIsland.exe`、`lyric-setup.exe`、`lyric-updater.exe`、`lyric-uninstall.exe`，以及 `package_preflight.py::MODULE_DLLS` 中列出的 15 个模块 DLL。
- Go：`LyricIsland.Center.exe`，必须以 `-H=windowsgui` 构建，且与仓库内存在的 Go 构建哈希一致。
- 资源：Assets、ShaderCache/Hybrid29、licenses、SQLite DLL、Inno Setup 编译器、安装脚本和随包文档。
- 发布说明须先复制到全新的输出目录；不能覆盖已经存在的运行目录。
- 新运行包禁止出现 `LyricIsland.Settings.exe` 和 `dotisle_settings.dll`。

当前构建命令见 [BUILDING.md](../docs/BUILDING.md)，架构见 [UNIFIED-CONTROL-CENTER.md](../docs/UNIFIED-CONTROL-CENTER.md)。

## 验收

`build/verify_packaged_ui.py` 运行包内 Center 的生产视图截图，然后通过真实命名管道检查一个 Center 加载桌面 DLL、组件添加/缩放/删除、控制中心重新唤起、布局持久化和退出重启。

其他门禁包括源码策略、Go 与 Rust 测试、计时器集成、模块加载、安装器/便携包和 SHA256 清单。截图生成后仍需人工查看；报告成功不能替代外观检查。

## 发布附件

Action 保留完整验收证据作为工作流产物。公开 Release 只保留安装版、便携包、源码包、校验清单以及旧命名便携包和校验清单。`prepare_public_release.py` 在上传或发布前筛选公开文件；`verify_prebuilt_release.py` 先核验内部证据。

3.5.x 旧更新器对旧设置文件的硬要求仍需单独处理。没有完成跨版本升级验收前，不应发布去除旧设置的新 OTA 包。
