# 点屿 3.1.8 取色回归

构建沿用 BUILD-3.1.7.md。运行 tools/li-cargo.ps1 test --workspace --offline 与 build --release --workspace --bins --examples --offline。

主程序 --immersive-preview <目录> 会调用生产渲染器生成 transport-content-* 截图，并在 report.json 的 transportCases 记录合成背景、选中图标颜色和最低对比度。六种封面色、四种材质组合与三种交互状态共 72 组，另含无封面场景；图标不允许回退到 #000000。domain 单元测试另外覆盖用户截图的实际背景色和交互层对比度。

本地输出使用版本 3.1.8，安装器 QA AppId 为 DotIsle.QA.Release318。未修改 README 下载链接和更新清单。
