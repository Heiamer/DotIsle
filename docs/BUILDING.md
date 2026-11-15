# 构建点屿 5.0

控制中心使用 Go + MyGo，主程序和功能模块使用 Rust。普通用户只需运行发布包，无需安装构建工具。

## 环境

- Windows x64、Git、Rust stable GNU 工具链和锁定依赖。
- Go 版本以 `Go/settings-center/go.mod` 为准。
- LLVM 的 `llvm-dlltool.exe`、`llvm-ar.exe`，以及 Zig C 编译器。
- 打包需 Inno Setup，资源和字体由仓库提供。

在仓库根配置工具和输出位置，使用 `tools/li-cargo.ps1`，由脚本设置链接器、静态 CRT 和本地库路径：

```powershell
$env:DOTISLE_TARGET_DIR = "$pwd/target"
$env:DOTISLE_DLLTOOL = 'C:/Tools/LLVM/bin/llvm-dlltool.exe'
$env:DOTISLE_ZIG = 'C:/Tools/Zig/zig.exe'
& ./tools/li-cargo.ps1 build --release --workspace --bins --lib --locked
```

依赖已缓存时可加 `--offline`。该构建生成主程序、桌面 DLL、其他功能模块及安装/更新辅助程序，不再生成旧设置程序。

然后在 Go 项目目录构建唯一控制中心，输出到同一 binary-dir：

```powershell
Push-Location Go/settings-center
$env:GOTOOLCHAIN = 'local'
go test ./...
go vet ./...
go build -trimpath -ldflags '-s -w -H=windowsgui' -o ../../target/release/LyricIsland.Center.exe .
Pop-Location
```

若 `Go/settings-center/LyricIsland.Center.exe` 中还保留先前构建，打包前须同步为当前构建；脚本会拒绝两份文件哈希不同的输入。

## 测试与本地打包

```powershell
$env:DOTISLE_MODULE_DIR = "$pwd/target/release"
$env:DOTISLE_TEST_SQLITE3 = "$pwd/build/e_sqlite3.dll"
& ./tools/li-cargo.ps1 test --workspace --locked
python build/verify_release_source.py --version 5.0.0-rc2 --report validation/source/report.json
```

某些 Windows 注册表与命名管道测试需要正常桌面会话权限。所有集成测试使用专用临时数据和测试管道。

先把 `release-notes/DotIsle-<版本>-Notes.md` 复制到新的输出目录，然后运行：

```powershell
python build/package_preflight.py --binary-dir target/release --output publish/local --version 5.0.0-rc2 --shader-cache Assets/ShaderCache/Hybrid29 --iscc C:/Tools/InnoSetup/ISCC.exe
python build/package_release.py --binary-dir target/release --output publish/local --work-dir publish/package-work --version 5.0.0-rc2 --shader-cache Assets/ShaderCache/Hybrid29 --iscc C:/Tools/InnoSetup/ISCC.exe
python build/verify_packaged_ui.py --runtime publish/local/DotIsle-5.0.0-rc2 --output validation
```

版本号需与仓库和发布说明一致。输出目录与 package-work 应全新创建，避免混入旧程序。`package_release.py` 生成安装版与两种命名的便携包；源码归档、模块包及校验清单由发布流程另行生成。

包内验证调用实际生产 Go 页面和 Rust 桌面渲染器，并进行真实管道/进程集成测试。还需查看截图、检查首次启动和隐藏/唤起，并运行计时、模块、安装器与升级验收。当前架构及旧 OTA 边界见 [UNIFIED-CONTROL-CENTER.md](UNIFIED-CONTROL-CENTER.md)。
