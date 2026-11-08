# 点屿 3.2.4 构建与发布

发行版本由 `.github/workflows/windows-release.yml` 在 `windows-latest` 上构建。已通过的构建见 [Actions 记录](https://github.com/Heiamer/DotIsle/actions/workflows/windows-release.yml)，对应源码标签为 `v3.2.4`。

## 使用 GitHub Actions

在 Actions 中打开 **Build and package Windows release**，选择源码分支后点击 **Run workflow**：

- `version`：新的三段式版本号，不带 `v`。
- `publish=false`：构建并上传草稿附件，便于先检查；`publish=true` 在构建后公开发布。
- 同名 Release 已存在时工作流会停止，不会覆盖现有附件。请使用尚未占用的版本号。

工作流安装 GNU Rust、经过 SHA256 校验的 **Zig 0.13.0**、LLVM 和 Inno Setup，依次运行源码版本与许可检查、9 项 Node 页面读取检查、工作区测试、发行编译、安装包与便携包打包、源码归档、SHA256 计算和渲染检查。

GPU 渲染检查生成控制中心、主题、背景及原图/进度报告；发布前的独立关卡要求四份界面报告和一份源码检查报告均存在且为 `success: true`。缺少报告或检查失败时停止发布，不以整体编译完成代替渲染验收。

## 本地构建

准备 Windows x64、Git、`stable-x86_64-pc-windows-gnu` Rust 工具链、Zig 0.13.0 和提供 `llvm-dlltool.exe` 的 LLVM。打包另需 Python 3 与 Inno Setup。

在仓库根目录运行；将示例路径替换为本机实际安装位置：

```powershell
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
$env:DOTISLE_ZIG = 'C:\Tools\zig\zig.exe'
$env:DOTISLE_DLLTOOL = 'C:\Program Files\LLVM\bin\llvm-dlltool.exe'

powershell -NoProfile -ExecutionPolicy Bypass -File tools/li-cargo.ps1 test --workspace --locked
powershell -NoProfile -ExecutionPolicy Bypass -File tools/li-cargo.ps1 build --release --workspace --bins --locked
```

默认二进制位于 `target/release`；可用 `DOTISLE_TARGET_DIR` 指定其他输出目录。不要在 `Rust` 目录中绕过启动脚本编译，它还负责配置静态 CRT、链接器和本仓库原生支持库路径。

## 打包

下面的目录仅作示例，运行前确认不存在需要保留的同名产物：

```powershell
New-Item -ItemType Directory -Force publish/release-output | Out-Null
Copy-Item release-notes/DotIsle-3.2.4-Notes.md publish/release-output/
python build/package_release.py --binary-dir target/release --output publish/release-output --work-dir publish/package-work --iscc 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' --version 3.2.4 --shader-cache Assets/ShaderCache/Hybrid29
```

源码 ZIP、验证附件和最终 SHA256 清单的生成方式见工作流。请保留 `Assets/Fonts`、`Assets/Themes`、`Assets/TransportIcons`、随包字体许可和第三方许可；不要把用户 `Data` 或 `target` 放入发行包。
