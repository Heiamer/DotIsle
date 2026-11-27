# AllegroSkia

独立的 Windows 桌面小组件与效率侧边栏。与上一级 `Rust/` workspace 无依赖。

使用 MSVC Rust stable、Skia CPU Raster、顶向下预乘 BGRA DIB 和 `UpdateLayeredWindow(ULW_ALPHA)`。目标为 Skia、文字布局、SkSL 和 CRT 静态链接；最终 DLL 导入表尚待实编验收，不能据配置推断已经满足零外部 DLL。

```powershell
$env:CARGO_HOME = "$PWD\.build\cargo-home"
$env:CARGO_TARGET_DIR = 'C:\Users\miku\.cache\dotisle-allegro-skia-target'
cargo +stable-x86_64-pc-windows-msvc test
cargo +stable-x86_64-pc-windows-msvc run -- --demo --render-preview "$env:TEMP\allegro-skia-smoke"
cargo +stable-x86_64-pc-windows-msvc build --release
```

当前沙箱构建受权限阻塞，详见 `HANDOFF.md`。本目录沿用了前序的本地下载桥接配置，联网构建前需单独运行 `node tools/download-bridge.cjs`；桥接走本机 7897 代理，仅为构建工具，不是运行时依赖。正常网络环境可以移除 `.cargo/config.toml` 的 source 替换段。

`Alt + ~` 展开或收起侧边栏，`Alt + E` 切换桌面编辑模式。编辑模式拖动桌面卡片后按 192 DIP 网格吸附。右键卡片可调整尺寸、材质、顺序、堆叠或删除。堆叠上滚轮切页；侧边栏其余区域滚轮滚动。点击搜索框后支持键盘与输入法，回车执行首项结果。`/light /dark /edit /add /quit` 为可用指令，数学结果回车写入系统剪贴板。

`--demo` 使用内存演示配置；`--keep-alive` 收起后保持常驻；`--data-dir <dir>` 指定两个独立 JSON 配置位置；`--pipe <name>` 仅预留通道标识，目前安全降级为离线模式。默认数据目录为 `%LOCALAPPDATA%\DotIsle\AllegroSkia`。错误日志位于 `%LOCALAPPDATA%\DotIsle\allegro-skia-startup.log`。

前端文件集中在 `src/ui/`，绘制函数接收只读 `View`，返回 `HitItem`。所有持久化、壁纸解码、开始菜单索引、剪贴板读写和系统采样都在命名后台线程执行，前端以 `try_lock` 读取内存快照。系统采样周期为 1.5 秒，不支持的数据保持未知，不填充模拟读数。演示模式不会保存配置，但应用启动和复制等明确点击操作仍访问系统。

该版本提供基础材质和交互；专注卡片当前为静态展示。壁纸为启动时读取的一张静态缓存，尚无壁纸变更通知。侧边栏只定位到主显示器，桌面以 `HWND_BOTTOM` 锚定，无 Explorer WorkerW 重挂载。命名管道仅预留、未连接主进程。高分辨率预览可独立离屏生成，不会进入 GUI 事件循环。
