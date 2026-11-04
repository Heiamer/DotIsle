# 点屿 3.2.0 构建与发布

## GitHub Actions（推荐）

仓库的 `.github/workflows/windows-release.yml` 支持手动触发：

- `version`：`3.2.0`
- `publish`：先保持 `false` 生成草稿，核对附件后再发布

工作流在 `windows-latest` 上安装 GNU Rust 工具链、Zig 0.17、LLVM 与 Inno Setup，然后执行：

1. `tools/li-cargo.ps1 test --workspace`
2. `tools/li-cargo.ps1 build --release --workspace --bins`
3. `python build/package_release.py ... --version 3.2.0 --shader-cache Assets/ShaderCache/Hybrid29`
4. 生成 Source.zip、Validation.zip 与 SHA256，上传到草稿 Release

## 本地构建

与 3.1.8 相同，使用 `tools/li-cargo.ps1`（GNU toolchain + Zig + LLVM dlltool）：

```
tools/li-cargo.ps1 test --workspace
tools/li-cargo.ps1 build --release --workspace --bins
python build/package_release.py --binary-dir target/release --output <输出目录> \
  --work-dir <临时目录> --iscc <ISCC.exe> --version 3.2.0 \
  --shader-cache Assets/ShaderCache/Hybrid29
```

打包前把 `release-notes/DotIsle-3.2.0-Notes.md` 放到输出目录。安装器使用 `build/installer.iss`，AppVersion 为 3.2.0。

包内包含新增的 `Assets/TransportIcons` 与 `Assets/ShaderCache/Hybrid29`；第三方图标来源与许可见 `Assets/TransportIcons/NOTICE.md`。
