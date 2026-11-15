> 2026-11-14 审查更新：r5 已打包并通过本地验收，位置见 HANDOFF 顶部。Center 已强制打包并核对哈希；CloseApplicationsFilter 已包含 Center。本文旧版本“缺工具/未打包”描述仅作历史记录。

# 打包链现状与待办（DotIsle 5.0.0）

本机实测时间 2026-11-14，命令与结论都可用 `build/package_preflight.py` 复现。

## 1. 本机现状

`python build/package_preflight.py --binary-dir "Rust\target\release" --output "$env:TEMP\dotisle-preflight" --version 5.0.0 --shader-cache "Assets\ShaderCache\Hybrid29" --iscc "C:\Program Files (x86)\Inno Setup 6\ISCC.exe"`
→ `合计 46 项：OK 39，MISSING 7`，退出码 1。

| 输入 | 状态 |
| --- | --- |
| `--binary-dir` = `Rust\target\release` | 存在（不是 CI 用的仓库根 `target/release`） |
| `LyricIsland.exe` / `lyric-island-settings.exe` | 在，303,104 / 304,128 字节 |
| `lyric-updater.exe` / `lyric-uninstall.exe` / `lyric-setup.exe` | 缺 |
| `LyricIsland.Center.exe` | 缺（`Go/settings-center/LyricIsland.Center.exe` 是 Lead 的截图验证产物，不要覆盖、不要当交付件） |
| `dotisle_*.dll` | 14/15，缺 `dotisle_glow.dll` |
| `--shader-cache` = `Assets\ShaderCache\Hybrid29` | 在，13 个文件 |
| 仓库 `Assets/`(195 文件) / `licenses/`(29) / `docs/brand/`(3) | 在 |
| `build/e_sqlite3.dll` / `build/installer.iss` / `build/ChineseSimplified.isl` | 在 |
| 根目录 8 个随包文档（README/LICENSE/CHANGELOG…） | 在 |
| `build/pe_resources.py` 可导入 | 是，`embed_icon` / `update_resources` 就绪 |
| `--iscc` | 缺：本机没装 Inno Setup，`C:\Program Files (x86)\Inno Setup 6\ISCC.exe` 不存在 |
| `release-notes/DotIsle-5.0.0-Notes.md` | 在 |
| `--output/DotIsle-5.0.0-Notes.md` | 缺：打包前必须先复制到输出目录 |

## 2. `build/package_release.py` 的完整输入清单

- `--binary-dir`：`LyricIsland.exe`、`lyric-island-settings.exe`、`lyric-updater.exe`、`lyric-uninstall.exe`、`lyric-setup.exe`（安装引擎）、`LyricIsland.Center.exe`（MyGo 控制中心）
- `--binary-dir`：15 个模块 DLL —— `dotisle_core` / `settings` / `focus` / `todos` / `music` / `lyrics` / `notifications` / `history` / `automation` / `devices` / `osd` / `sound` / `widgets` / `themes` / `glow`（第 45 行，含 `dotisle_glow.dll`）
- `--shader-cache`：着色器缓存目录（CI 传 `Assets/ShaderCache/Hybrid29`）
- `--iscc`：可执行的 `ISCC.exe`
- `--output`：输出目录，且 `<output>/DotIsle-<版本>-Notes.md` 必须在打包**之前**就存在（第 56 行）
- `--work-dir`：临时目录（安装器 stage 与 InstallPayload.zip），必须是空的/全新的
- 仓库内：`Assets/`、`Assets/LyricIsland-v3.ico`、`licenses/`、`docs/brand/`、`build/e_sqlite3.dll`、`build/installer.iss`、`build/ChineseSimplified.isl`、`build/pe_resources.py`（可导入）
- 仓库根随包文档：`README.md`、`GESTURE-TUTORIAL.md`、`NETEASE-SEEKFIX.md`、`CHANGELOG.md`、`LICENSE`、`FONT-NOTICE.md`、`THIRD-PARTY-NOTICES.md`、`REWRITE.md`
- `--version`：与 `Rust/Cargo.toml` 的 `workspace.package.version`（5.0.0）一致

注意第 43 行的 `if center.is_file()`：控制中心缺失是**静默跳过**，所以必须由 preflight + CI 断言兜住。

## 3. CI 修复点摘要（`.github/workflows/windows-release.yml`）

1. Go 步骤输出路径改为 `$env:GITHUB_WORKSPACE/target/release/LyricIsland.Center.exe` 并 `New-Item` 建父目录，与打包步骤的 `--binary-dir "$pwd/target/release"` 对齐（原来落在 `Go/settings-center/target/release`）。
2. build 后断言文件存在且非空；补 `gofmt -l`（必须净）、`go vet ./...`、`go test ./...`（原来只测 `./ui/`）。
3. Go 压缩包加 SHA256 校验（与 Zig 步骤同标准）；钉 `GOTOOLCHAIN=local`，避免代理偷偷换工具链。
4. 打包步骤前插入 `python build/package_preflight.py ...`（参数与 CI 既有变量一致）。
5. 打包步骤后新增硬门禁：`publish/release-output/DotIsle-<版本>/LyricIsland.Center.exe` 必须存在且非空，且 15 个 `dotisle_*.dll` 齐全。
6. `git archive` 补退出码与产物存在性检查（原来 Source.zip 缺失也不报错）。
7. `workflow_dispatch` 的默认版本 `3.5.1` → `5.0.0`（与 `Assets/UpdateManifest.json` 一致，否则首步 verify 必挂）。

## 4. 要真正打出安装包还差什么

1. 装 Inno Setup 6（或把任意可用的 `ISCC.exe` 绝对路径记下来）。
2. 补 Rust 二进制到同一个 target 目录（`li-cargo.ps1` 默认写仓库根 `target/release`；想复用现有 `Rust/target/release` 就先设 `DOTISLE_TARGET_DIR`）：
   `powershell -NoProfile -ExecutionPolicy Bypass -File tools/li-cargo.ps1 build --release --workspace --bins --lib --locked`
   这一步同时产出 `lyric-setup.exe`、`lyric-updater.exe`、`lyric-uninstall.exe` 和 `dotisle_glow.dll`。
3. 产出控制中心到同一个 `--binary-dir`（用临时输出，别覆盖 Lead 那份）：
   `cd Go/settings-center; go build -trimpath -ldflags "-s -w -H=windowsgui" -o <target>/release/LyricIsland.Center.exe .`
4. 准备输出目录并先放发布说明：`New-Item -ItemType Directory -Force <out>`；`Copy-Item release-notes/DotIsle-5.0.0-Notes.md <out>`。
5. 跑 preflight，必须 `MISSING=0`。
6. 跑打包：`python build/package_release.py --binary-dir <target>/release --output <out> --work-dir <全新空目录> --iscc <ISCC.exe> --version 5.0.0 --shader-cache Assets/ShaderCache/Hybrid29`
7. 后续与 CI 一致：`python build/package_modules.py --runtime <out>/DotIsle-5.0.0 --output <out> --version 5.0.0`，再按需跑 `build/verify_packaged_ui.py`、`tests/verify_pomodoro.py`、`tests/verify_modules.py`。
8. 已知非阻塞项：`build/installer.iss` 的 `CloseApplicationsFilter` 没有列 `LyricIsland.Center.exe`，升级安装时若控制中心正在运行，Inno 不会自动关闭它，可能出现文件占用失败。
