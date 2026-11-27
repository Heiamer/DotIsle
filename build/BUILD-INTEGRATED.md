# 本地构建集成版本

先按 Go/settings-center/build.cmd 的现有 MyGo 修补流程构建控制中心。Rust 主程序继续通过 tools/li-cargo.ps1 构建，不改液态玻璃源码。

侧边栏目录使用 Visual Studio x64 MSVC 环境运行 cargo build --release --locked；将其 release/DotIsle-Allegro.exe 复制到与主程序相同的 binary-dir。使用该程序 --render-preview Assets/Allegro 生成组件缩略图。

然后运行 build/package_preflight.py 和 build/package_release.py；新打包脚本会将侧边栏引擎和预览资源纳入运行清单、安装包及便携包。需指定 Inno Setup 编译器、Hybrid29 着色器缓存和全新输出/工作目录。

本轮验证与已知限制见交付目录测试说明.md。不要使用真实用户 Data 运行回归测试。
