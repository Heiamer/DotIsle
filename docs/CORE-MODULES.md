# CORE 与内部 DLL 模块

## 结构

启动器和独立更新器保留为 EXE；CORE 本身可替换。固定 C ABI 在 DLL 边界传递长度明确的 UTF-8 JSON，模块分配的响应必须由同一个模块释放，不跨边界共享 Rust 对象或分配器。

| 文件 | 职责 |
| --- | --- |
| LyricIsland.exe | 小启动器、CORE 启动 ABI/角色检查 |
| dotisle_core.dll | 窗口、消息循环、公共场景渲染、服务编排与配置入口 |
| LyricIsland.Settings.exe / dotisle_settings.dll | 控制中心与公共设置界面渲染 |
| dotisle_focus.dll | 计时状态机、秒表、流程、暂停恢复与模板 |
| dotisle_todos.dll | 待办、提醒标记、稍后提醒和快捷语句解析 |
| dotisle_music.dll | 播放器采样、媒体命令、时间线时钟与封面 |
| dotisle_lyrics.dll | 歌词检索、格式解码、匹配、缓存和逐字同步 |
| dotisle_notifications.dll | 通知接收、过滤、只读数据库及图标读取 |
| dotisle_history.dll | 历史记录与保留策略 |
| dotisle_automation.dll | 按应用提醒、使用时长规则与计数 |
| dotisle_devices.dll | 音频、USB、手柄和输入设备连接提醒 |
| dotisle_osd.dll | 锁定键、静音和触控板状态提示 |
| dotisle_sound.dll | 音效选择、校验和播放 |
| dotisle_widgets.dll | 桌面组件配置、统计、天气、时区和设备指标 |
| dotisle_themes.dll | 取色、主题角色、对比度和配色过渡 |
| dotisle_glow.dll | 光效状态、音量强弱响应和绘制指令 |
| LyricIsland.Updater.exe | 独立校验、安装、备份、回滚和启动确认 |

公共模型、几何和基础渲染库属于宿主基础设施。业务状态机不再以静态实现藏在 CORE 中：宿主里的 timers/todos 等文件是实际调用 DLL 的适配器。界面排版仍由 CORE 的公共岛渲染与控制中心 DLL 负责；修改页面排版需要更新对应的界面模块，不能声称换一个业务 DLL 就能改变所有界面。

这是内部模块体系，没有第三方插件扫描、插件目录或插件选项卡。

## 从设置替换

设置首页顶部 → 模块管理 → 对应模块的“选择替换 DLL”。CORE、控制中心及十三个功能 DLL 均列出版本。校验通过后点击“安装并重启”。

校验在独立子进程中运行，包括加载、身份、C ABI、CORE API 范围、依赖和无副作用接口自检；校验失败不会覆盖运行中的文件。DLL 只在应用退出后替换，不进行进程内卸载。更新助手验证控制中心可以构建实际界面，并确认新主进程的模块版本及初始化状态。失败会恢复原模块并重启旧程序。

每个模块保留一个上一版本，可从同页暂存恢复，再安装并重启。首次迁移必须使用新的启动器、CORE 和完整 DLL 集合，不能把 DLL 直接扔进从未支持此接口的旧版 EXE。

## 单模块构建与缓存

使用已有 GNU/Zig 工具链，并设置 DOTISLE_DLLTOOL、DOTISLE_ZIG。示例：

```powershell
tools/build-module.ps1 -Module Focus -Profile fast
tools/build-module.ps1 -Module Lyrics -Profile release
tools/build-module.ps1 -Module Core -Profile release
```

Module 支持 Core、ControlCenter、Updater 及上述十三个业务名称。共享构建缓存默认为 Documents/Codex/BuildCache/DotIsle，通过 -CacheRoot 可指定已有缓存；链接支持库使用固定路径，避免每份源码副本重新生成一套依赖缓存。

fast profile 关闭 LTO、启用增量编译和较低优化；release 用于最终验收。首次建立缓存仍需编译依赖；修改公共接口或编译器也仍会重建受影响部分。不要日常 cargo clean，也不要为了换图片改 workspace 版本。各功能 crate 的版本独立，公共库不随发行号机械升级。

Focus 的 module-probe / module-broken 是 CI 验收专用 feature，分别证明旧 CORE 调用了新功能 DLL，以及启动失败会自动回滚；不要把这些测试 DLL 用作正式包。

## 资源包与发行

字体、图片、SVG、音效等作为外部资源；音效和播放按钮 SVG 不再通过 include_bytes/include_str 嵌入功能 DLL。更新后重启生效。

```powershell
python build/package_modules.py --runtime path/to/runtime --output path/to/release --version 3.5.2 --runtime-version 3.5.1
```

命令中的版本仅为示例。发行包版本与实际编译的 CORE 版本分别表达。此命令只压缩已有运行时，不调用 Cargo。

更新器校验官方索引摘要，逐文件比较 SHA256，只下载变化的包；检查 ZIP 路径、包摘要、文件摘要及整个待安装模块集合的兼容性。然后沿用独立事务备份、退出安装、启动确认与失败回滚。没有模块索引的旧发行版仍走全量包通道；已存在但摘要错误的索引拒绝安装，不绕过校验。

模块下载当前使用 GitHub 官方附件；既有全量通道仍有备用源。这里沿用官方 HTTPS 源加摘要的信任模型，不是离线数字签名。目前不自动删除发行清单之外的旧文件，不支持无约束的跨 API 或数据 schema 替换。

## 接口与开发约定

参见 CORE-MODULE-ABI.h 和 dotisle-module-api。C ABI 与 CORE 服务 API 分开版本化；当前均为 1。模块描述包含 id、version、abi、coreApi、coreMin、coreMax、dataSchema、dependencies。

每个模块通过 describe 返回元数据，通过 health 提供不接触真实用户文件和设备输入的兼容自检。业务会话由模块持有，宿主仅保存句柄和可序列化显示状态；handle 由拥有它的模块销毁。后台音乐与歌词任务有各自的并发、截止时间及取消路径。光效向公共渲染器返回受限绘制指令，不传 Vello/Rust 对象指针。

新增兼容命令可以保持 API；破坏旧命令、枚举或数据格式时必须提高对应要求，并成组更新受影响的 CORE、模块和配置迁移逻辑。不能因为文件扩展名是 DLL 就假定它能替换所有旧版本。

## 验收

Actions 在独立测试分支运行：完整构建、模块单元回归、设置及五主题截图、计时与原生效果回归、资源包确定性测试，以及真实管道上的 DLL 替换和启动失败回滚。verify_modules.py 要求只替换 Focus DLL 就改变行为，同时 CORE 摘要完全不变。通过前不发布或替换正式程序。
