> 历史迁移评审：本文中的旧设置回退及独立 Rust 宿主已删除。当前结构见 [统一控制中心](../../docs/UNIFIED-CONTROL-CENTER.md)。

# MyGo 控制中心 — 代码审查交接文档

> 审查对象：`Go/settings-center/`（新增）+ `Rust/crates/` 六个文件的少量修改。
> 本文档写给没有上下文的审查者（人或模型），包含全部背景、契约、风险点与验证方法。
> 基线：`DotIsle-5.0.0-Source.zip`（原封未动的 5.0.0 源码），审查 diff 时可用它对照。
> **第四轮补测（2026-11-14 晚）尚未收录进本文；见 `HANDOFF.md` v4 §5.1c/§6.1（processAlive、管道 overlapped、模块页接通、按钮 enabled）。**

---

## 1. 任务背景与目标

DotIsle（点屿）5.0.0 的控制中心（设置窗口）原为 Rust + winit + vello 自绘实现
（`Rust/crates/dotisle-app/`，约 1.5 万行 UI 代码）。本次任务按《MyGo Native UI 正式开发
指令》用 **MyGo Native UI（Go，Direct3D11 渲染，无 WebView）** 重写控制中心界面层：

- **只重写 UI 层**：所有业务语义（配置写入、播放控制、待办/计时、通知历史…）仍由
  主程序 `LyricIsland.exe`（下称 island）通过既有命名管道承担，Go 端不做业务逻辑复制。
- **桌面组件保持原方案**：桌面组件窗口仍由 Rust 设置进程（desktop_host.rs）渲染，
  Go 版的"组件页"只通过管道命令管理。
- **调色板零偏差**：Go 通过 syscall 直接调用 `dotisle_themes.dll` 的稳定 C ABI，
  不移植 material-colors 算法。
- 主题皮肤（Miuix/Terminal/Pixel/Gallery）按产品决定**不移植**，Go 版固定 Original。

## 2. 架构与数据流

```
┌────────────────────────┐      ┌─────────────────────────────┐
│ LyricIsland.Center.exe │      │ LyricIsland.exe (island)     │
│ (Go, MyGo Native UI)   │      │ Rust + vello + D3D11         │
│                        │      │                              │
│  ui/      视图层        │─────▶│ lyric-backend/src/ipc.rs     │
│  island/  管道客户端    │ IPC  │   管道服务端（单实例）        │
│  themes/  调色板桥      │      │   \\.\pipe\LyricIsland.      │
│  prefs/   本地存储      │      │        Native.Settings.v1    │
└────────────────────────┘      └─────────────────────────────┘
        │                                │            ▲
        ▼ LoadLibrary                    │            │ 通知/唤醒事件
┌────────────────────────┐      ┌────────┴────────────┴─┐
│ dotisle_themes.dll     │      │ lyric-native main.rs   │
│ (调色板算法，与 vello  │      │ 业务：SMTC/网易云、    │
│  UI 同一算法)          │      │ 歌词、计时、待办、模块  │
└────────────────────────┘      └────────────────────────┘
```

- Go ↔ island：换行分隔 JSON 请求/响应（`{"version":1,"command":C,"value":V}`）。
- 启动链：运行包内存在 `LyricIsland.Center.exe` 时，island 的 `show_settings_page`
  优先拉起它（Rust 侧 `show_settings_page` 已改）；否则回退原版 `LyricIsland.Settings.exe`。
  `LyricIsland.Settings.exe --warm` 继续承担桌面组件宿主，与 Go 窗口互不干扰。

## 3. 文件地图（Go，约 9800 行）

| 文件 | 行数 | 职责 | 对应 Rust 来源 |
|---|---|---|---|
| `main.go` | 64 | 启动：flag/单实例/字体/窗口/管道泵 | dotisle-app/src/main.rs |
| `singleinstance_windows.go` | 38 | 控制中心单实例互斥 | platform.rs center_instance |
| `island/island.go` | 321 | **管道客户端**：重试规则、唤醒事件、拉起 island、进程监测 | lyric-backend/io.rs + dotisle-app/remote.rs |
| `themes/themes.go` | 232 | **LoadLibrary dotisle_themes.dll** 的 C ABI 桥（Win64 sret） | dotisle-module-api/lib.rs |
| `prefs/prefs.go` | 148 | preferences.json（schema 3）读写 + 原子写 | dotisle-domain/preferences.rs |
| `ui/theme.go` | 198 | 调色板装配（resolve_center → roles.ui → 中性覆写）+ 布局常量 | model.rs scheme/dynamic_color |
| `ui/kit.go` | 277 | 控件库：Label/Panel/Button/Chip/Switch/ToggleRow/Slider | widgets.rs Original 分支 |
| `ui/root.go` | 507 | App 状态 + 窗口框架（标题栏/头部/内容/导航/弹窗钩子） | views.rs render + shell.rs |
| `ui/pages.go` | 236 | 设置中枢路由（tab 255/240-246/0-9/128-134/135）+ 链接卡 | settings_hub.rs |
| `ui/pages_home.go` + `_cards.go` | 537 | 概览甲板：环形数学（card_ring.rs）+ 五张卡 | overview.rs + music_card.rs |
| `ui/pages_music.go` | 397 | 音乐页：传输键（Reef）、进度拖动、逐字歌词、±100ms | views.rs fn music |
| `ui/pages_widgets.go` | 1048 | 组件画廊：15 类 × 3 尺寸 × 5 风格真实预览 + 桌面组件管理 | desktop_cards.rs gallery |
| `ui/pages_island.go` | 568 | 灵动岛 7 个子页（128-134）全部行构建器 | island_settings.rs |
| `ui/pages_todos.go` | 314 | 待办页 + 待办弹窗（todoSave 契约） | views.rs fn todos |
| `ui/pages_notifications.go` | 186 | 通知页（过滤 chips、历史行、权限横幅） | notification_center.rs |
| `ui/pages_focus.go` | 1828 | 计时页 + 流程编辑器 + 5 个弹窗 + 计时引擎复刻 | timer_surface.rs + workflow.rs |
| `ui/pages_leaves.go` | 1265 | 设置叶子页 tab0-9 全量 | preferences_ui.rs 等 |
| `ui/pages_dialogs.go` | 391 | 快速提醒 + 应用规则编辑弹窗 | views.rs dialog |
| `ui/colorpicker.go` | 272 | HEX 调色盘（HSV 网格/色相条/预设/保存联动） | picker.rs |
| `ui/fontpicker.go` | 202 | 字体选择器（注册表枚举 + IslandFontFamily） | font_picker.rs |
| `ui/palette.go` | 127 | Ctrl+K 搜索面板 | views.rs palette |
| `ui/signature.go` + `_routes.go` | 248 | 手写字标（7 笔迹路径解析 + 笔长揭示） | dotisle-render/brand_signature.rs |
| `ui/background.go` | 128 | 壁纸：加载缓存/Contain-Cover/亮度/顶部纱/导入 | background.rs |
| `ui/filedialog_windows.go` | 130 | GetOpenFileNameW + SHBrowseForFolderW（无 cgo） | platform.rs |
| `ui/dialogs_hook.go` | 115 | 弹窗渲染钩子 + toast + 三个文件选择入口 glue | — |
| `ui/app_state.go` | 186 | 快照访问器/计时器展示函数（timers.rs 复刻） | timers.rs |
| `ui/assets.go` / `icons.go` | 134 | 运行时字体注册 + MaterialSymbols 加载 | dotisle-render/draw.rs |
| `cmd/pipeprobe/main.go` | 170 | **端到端探针**：12 项管道契约检查 | — |

三方子代理写的文件（需额外仔细审查）：`pages_focus.go`、`pages_leaves.go`、
`pages_todos.go`、`pages_notifications.go`、`pages_dialogs.go`、`pages_widgets.go`（预览部分）。

## 4. Rust 侧改动（相对 5.0.0 原始 zip 的全部 diff）

原始 zip 在 `C:/Users/miku/Documents/Trae-CN/temp_20261113_01/DotIsle-5.0.0-Source.zip`，
可用 `unzip -p <zip> "DotIsle-5.0.0-Source/<path>"` 取基线逐文件 diff。

| 文件 | 改动 | 意图 |
|---|---|---|
| `lyric-native/src/main.rs` | ① `show_settings_page`：可见控制中心优先拉 `LyricIsland.Center.exe`（新字段 `center_process`，与 `settings_process` 分开跟踪）；② `"state"` 命令分支开头 `self.poll()`；③ `poll()` 末尾检测 `frame` 变化时 signal `SettingsWake`；④ 新字段 `settings_wake` | ① Go 前端接入；②③④ **歌词同步修复**（见下） |
| `dotisle-app/src/remote.rs` | 轮询间隔 240ms → 90ms（两处 wait/sleep） | 组件歌词与岛同帧 |
| `dotisle-app/src/desktop_cards.rs` | `render/render_style` 增加 `media_position: Option<f64>` 参数，透传 `ViewState`；画廊调用点传入 `state.media_position` | 组件进度条插值 |
| `dotisle-app/src/desktop_host.rs` | `event()` 增加 `media_position` 参数并传给 render | 同上 |
| `dotisle-app/src/shell.rs` | `timeline.tick` 提前到 `desktop.sync` 之前，`sync(changed \|\| media_moving)`；`window_event` 传 `seek_preview.or(timeline.position)` | 播放中组件按帧重绘 |
| `dotisle-app/src/model.rs` | `current_lyric()` 空行回退改为标题（对齐 compact::primary） | 两侧文案一致 |
| `dotisle-app/src/widget_checks.rs` / `widget_extras.rs` | `render_style` 新参数的调用点补 `None` | 签名变更 |
| `dotisle-app/src/main.rs` | 仅 `mod center_layout;` 声明位置移动（无害重排） | — |
| `CHANGELOG.md` | 5.0.0 条目下新增 4 行 | 发布纪律 |

**歌词同步修复背景**（对应以上 ②③④）：修复前桌面组件拿到的是岛端二手快照——
`"state"` 命令不刷新音乐快照 + 设置进程 240ms 轮询 → 组件歌词滞后 0.25~0.6s。
修复：state 应答前 poll + 歌词帧变化时 signal 唤醒事件 + 轮询对齐 90ms + 进度插值。

⚠️ 审查注意：`poll()` 在 `"state"` 分支被调用是**既有模式**（`testCompactFrame` 分支
早已如此），同线程（WM_SETTINGS 与 GUI 循环同线程），无新竞态。

已完全回滚的实验（不应出现在审查范围）：`Rust/crates/lyric-settings-gpui/` 已删除、
workspace members 已还原、dotisle-app 的 pub mod/WAKE_HOOK 已还原（`dotisle-app/src/main.rs`
仅剩上述无害重排）。

## 5. 关键契约（审查时逐条对照）

### 5.1 管道协议（island/island.go ↔ lyric-backend/src/io.rs + ipc.rs）

- 请求一行 JSON + `\n`；响应一行 JSON；请求 ≤16KiB、响应 ≤1MiB、读超时 8s。
- **重试规则**（`request()`）：只读 `state` 重试 4 次；变更命令 2 次；**变更已投递
  （写入成功）后绝不重试**——`tryPipe` 返回 `delivered` 标记，`request` 据此短路。
- 连接循环 2s：`WaitNamedPipeW(100ms)` → `os.OpenFile`；错误 2/231/535 可重试。
- 管道名：环境变量 `LYRICISLAND_TEST_PIPE` 覆盖，默认 `LyricIsland.Native.Settings.v1`。
- 唤醒事件：`Local\DotIsle.SettingsWake.{pipe}`（auto-reset），island 在歌词帧变化时
  signal（本次 Rust 修复新增），Go worker `WaitForSingleObject(90ms)`。
- **审查点**：`islandRunning()` 用 CreateMutex 探测，错误分支语义
  （ERROR_ALREADY_EXISTS / ACCESS_DENIED）是否在 golang.org/x/sys 下的行为与 Rust 一致。

### 5.2 命令与 payload（UI 发出的全集，均经 12 项探针验证）

| 命令 | payload | Rust 处理位置 |
|---|---|---|
| `state` | null | lyric-native main.rs `"state"` 分支 |
| `update` | `{"key","value"}` | main.rs update 分支（config::validate_setting） |
| `media` | `{"action":"toggle/previous/next/seek","seconds"?}` | main.rs media 分支 |
| `toggleVisibility` | null | main.rs（user_hidden 翻转） |
| `todoSave/todoComplete/todoDelete` | model.rs DialogKind::Todo 分支 payload | main.rs todo 分支 |
| `timerCreate` | `{"mode","duration","focus","rest","rounds"}` 或 workflow `{"mode":"workflow","name","stages":[{"name","seconds"}]}` | model.rs start_payload |
| `timerAction` | `{"id","action","revision"}`，action ∈ pause/resume/next/reset/**delete**/skip… | main.rs timerAction |
| `quickReminderPreview/Commit` | `{"text"}` / `{"previewToken","clientRequestId"}` | model.rs |
| `appRuleSave/appRuleAction` | model.rs 对应分支 | main.rs |
| `widgetAdd/widgetUpdate/widgetRemove` | `{"kind","size","style"?}` / `{"id","size"}` / `{"id"}` | main.rs widget 分支 |
| `historyOpen/historyClear` | `{"id"}` / null | main.rs |
| `moduleStage/moduleApply/moduleCancel/moduleRollback/moduleRefresh` | `{"id","path"}` / `{"callerSettingsPid"}` / null / `{"id"}` / null | main.rs |
| `soundSelect/soundPreview/deviceSoundPreview/resetBuiltinSounds` | 见 model.rs | main.rs |
| `pageSeen` | null | main.rs |
| `neteaseEnableSilent` | null | main.rs |

⚠️ **审查重点**：update 键必须是 island `config::validate_setting` 接受的键。已知的
教训：`IslandVisible` 不是合法键（静默忽略），显隐必须走 `toggleVisibility`——探针
曾抓出 Go 端这个 bug，已修（root.go 灵动岛开关）。审查时确认没有残留类似错误键。

### 5.3 调色板 DLL ABI（themes/themes.go）

- `dotisle_service_call(*const u8, usize) -> Buffer{ptr,len}`：**Win64 下 >8 字节结构体
  经隐藏指针返回**，Go 侧 `proc.Call(uintptr(unsafe.Pointer(&result)), ptr, len)`。
- `dotisle_service_free(Buffer)`：16 字节结构体按引用传递。
- 请求 `{"method":...,"args":...}`，响应 `{"ok":bool,"value"/"error"}`，describe 校验
  `abi==1 && id=="themes"`（与 dotisle-module-api/lib.rs 完全一致）。
- **审查点**：隐藏指针约定是否对所有调用方式成立；`unsafe.Slice` 拷贝时机与
  free 的先后（当前实现先 copy 后 free，正确）。

### 5.4 布局与渲染

- 所有页面为**绝对定位 float32 DIP**，常量来自 center_layout.rs / views.rs /
  settings_hub.rs / island_settings.rs / picker.rs / font_picker.rs（文件内有注释标注来源）。
- 文本行盒 1.35×size（parley 语义）；状态层 hover 0.08 / pressed 0.12 / focus 0.10。
- 中性色覆写（theme.go `override`）必须与 model.rs `Model::scheme` 的 mix 比例逐项一致。

## 6. 已知偏差与未完成（诚实清单）

1. **壁纸模糊**：Rust 的 CPU 高斯模糊未复刻（`background.go` 注释声明）， veil 用
   双段两停渐变近似四停渐变。
2. **组件预览内控件为静态绘制**：预览内的按钮/表盘不响应点击（Rust gallery 预览带
   活命中断）；Music 传输键未复刻 10 种 TransportTheme。
3. **手写字标**：写完阶段直接保留笔迹粗线，未做"实心轮廓 + 笔迹蒙版"的最终帧切换
   （Rust 用 kurbo::stroke 生成蒙版路径，MyGo 无 stroke-to-path）。
4. **手势教程**（island 页"再看一次"）未移植（tutorial.rs 是 5 步动画浮层）。
5. **字体列表**：注册表枚举与 fontique 的 system_families 可能存在少量差异
   （合并同族 style 项的规则是启发式）。
6. **信息卡渐变底近似纯色**；Pixel 风格预览未强制 Fusion Pixel 字体（系统回退）。
7. **甲板弹簧**：dt 固定 1/60（帧节流时略慢于真实时间）。
8. 调色盘 HEX 输入为普通 TextInput（无 Rust 版的 caret 手绘与按键拦截）。

## 7. 审查重点建议（按风险排序）

1. **island/island.go 重试与重放安全**：对照 io.rs 的 `pipe_request/pipe_attempt`。
   特别检查：变更命令在"写入成功但读响应失败"时是否真的不重试；连接循环的退出条件。
2. **themes/themes.go 的 Win64 sret 调用**：隐藏指针参数顺序、buffer 生命周期、
   以及 `proc.Call` 错误处理的噪音分支（当前对 `r==0 && callErr != EINVAL` 的判断
   逻辑值得审一遍——确认不会误吞错误）。
3. **ui/theme.go `override()`**：逐行对照 model.rs `Model::scheme`，检查 mix 目标色
   与比例（dark/light 反转处容易错）。
4. **并发模型**：Go 侧唯一跨线程点是 island worker → `win.Update`（MyGo 官方线程
   边界）；`command()` 从 UI goroutine 直接 `island.Request`（每次命令新连接，
   与 Rust 版常驻 worker 的差异——island 管道服务端是单实例串行，确认不会与
   worker 的 state 轮询死锁：服务端单请求循环下客户端并发会排队，最多等待 8s 超时）。
5. **三份子代理页面**：payload 格式抽查（todoSave/timerCreate/appRuleSave/quickReminder）
   对照 model.rs；`a.State` 取值的 nil 安全。
6. **preferences.json 原子写**：WriteFile(tmp)+Rename 与 Rust storage::atomic_write
   语义一致性（Windows rename 覆盖已存在目标的行为）。
7. **float32 布局换算**：全部坐标从 Rust f64 常量转来，检查 kit.go 的换算是否有
   截断导致的 1px 错位（截图验证未见，但静态审查可复核）。
8. **ui/kit.go Slider**（v2 重写后）：`sliderDrags` 以 label 为键——同 label 的多
   个滑条会互相干扰（当前页面无此用法，但这是潜在坑）。拖动预览 + 松手提交语义
   参照 pages_leaves.go `leafSettingsSlider`（settings_slider.rs 复刻）。
9. **signature.go 路径解析器**：手写 M/C/L/Z 解析，未处理 S/Q/A 与科学计数法数字
   （Rust 源数据确实只含 M/C/L/Z，但审查可确认）。本轮修复了 SVG 坐标之间空格/
   逗号分隔符未跳过的 bug（此前全部 y 值为 0 导致字标不可见）。
10. **签名笔迹蒙版**：`sigStrokeRegion` 用四边形 + 圆盘拼接模拟圆帽笔触——与
    kurbo::stroke 生成的真实圆帽可能存在微小几何差异（`ClipPath` 裁切后不可见）。
11. **pages_home.go 甲板**：拖拽层 z 序在卡片下方（先建 drag box 后建卡片）；空白
    处拖拽由 drag box 承接、卡片内拖拽由卡片自身接受——与 Rust 的全卡 hit 行为
    有差异（Rust 整卡是 OverviewDrag 目标，Go 版只有 focused 卡+露条可点击导航）。

## 8. 构建与验证

```
# Go 控制中心（需要 Go ≥ 1.27.3；仓库锁定 Go 1.26 + 工具链自动切换 1.27.4）
cd DotIsle-5.0.0-Source/Go/settings-center
set PATH=%USERPROFILE%\.zcode\workspace\draft-474987dd-cfa2-41c5-9a7b-e9118dbd5160\tools\go-sdk\go\bin;%PATH%
set GOPROXY=https://goproxy.cn,direct
go build -o LyricIsland.Center.exe .
go vet ./... && gofmt -l .

# Rust 侧（lyric-native 测试需要 debug 模块 DLL + zig cc 环境）
powershell -File <workspace>\dotisle-build-env.ps1 test -p lyric-native
#   预期：170 passed

# 端到端探针（12 项，需先起一个 island）
set LYRICISLAND_TEST_PIPE=LyricIsland.Settings.test.e2e
set LYRICISLAND_NATIVE_DATA=<隔离数据目录>
LyricIsland.exe &        # island
pipeprobe.exe            # 预期 {"fail":0,"pass":12}
```

- 依赖锁定：mygo v0.9.0（go.mod）、golang.org/x/sys、Go 1.26.11 工具链（自动切换 1.27.4）。
- `pipeprobe` 位于 `cmd/pipeprobe/`，全部断言基于 UI 实际发送的 payload。

## 9. 验证证据

- **全功能端到端测试 97/97 通过**（三遍确认）：`tests/dotisle_e2e.py`
  覆盖 68 个 update 键往返 + toggleVisibility + 待办/计时/规则/快速提醒/组件/音效
  全生命周期 + 应答型命令。隔离数据目录 + 独立测试管道，不影响真实运行实例。
- 逐页截图：设置中枢（6 色分组卡）、待办、通知、计时（4 模式 + 流程编辑器）、
  组件画廊（5 风格真实时钟预览）、灵动岛材质页、外观页、调色盘、字体选择器、
  概览甲板、启动字标——均与原版布局对照检查。
- Go center 对真实 3.5.1 正式版 island 连通验证通过（协议向后兼容）。
- `gofmt` / `go vet` 全净；Rust `cargo test -p lyric-native` 170/170
  （需 `DOTISLE_MODULE_DIR=target/debug` 与 zig cc 环境，见 dotisle-build-env.ps1）。

## 10. 本轮修复的 bug 清单（e2e 发现 → 已修）

| 发现 | 根因 | 修复 |
|---|---|---|
| 灵动岛开关无效 | Go 发 `update IslandVisible`（island 无此键） | 改走 `toggleVisibility` 命令（model.rs Pref("island")） |
| 组件预览 kind 无效 | Go 发 `"Clock"`（Rust serde camelCase 应为 `"clock"`） | 修正 homeWidgetKind + widgetKindLabels 全部 15 项 |
| 字标不可见（全 y=0） | SVG 解析器未跳过坐标间的空格/逗号分隔符 | `read()` 前置空白/逗号跳过循环 |
| 字标写完后仍为粗线条 | 只画了笔迹，没有实心轮廓被蒙版揭示 | 加载 7 个轮廓 + `Painter.ClipPath(笔迹蒙版){FillPath(轮廓)}` |
| 概览甲板点击无效 | drag box 创建在卡片循环之后（z 序覆盖覆盖层） | 拖拽层移到卡片循环之前 |
| 概览点击导航被覆盖 | 空闲吸附每帧都跑，覆盖点击设置的 deckTarget | 吸附加 160ms 输入静默门控（scroll_input 语义） |
| 甲板基准偏移 | 未加 overview.rs 的 `top = clip.y0 + 54` 偏移 | 甲板 top 固定 54 |
| 设置滑条拖动回弹 | 每帧新建 `live` 局部变量绑定 SliderBase | kit.Slider v2：拖动预览 + 松手提交 + 快照同步 |

## 10. 第二轮真机验证与修复（2026-11-14 下午，接力交接包 §7）

> 证据均在 `%TEMP%`（dotisle-verify 数据目录 + dk*/fp*/sp*/sq*/u*/v*/w*/x*/y* 截图 + ui-trace.txt + settings.json/todos.json 落盘核对）。

### 10.1 本轮修复的缺陷（全部真机复验）

| # | 缺陷 | 根因 | 修复 | 验证 |
|---|---|---|---|---|
| 1 | 页 131 七个滑条区间/默认值与 island 校验不一致，越界提交被静默拒绝（拖后回弹） | Go 端参数与 config.rs validate_setting / island_settings.rs 原版行不一致 | pages_island.go 对齐：MotionDuration 480/250–1200、Collapse 440/150–700、Gap 8/0–24、Mass 0.8/0.3–2、Stiffness 420/100–1000、Damping 30/10–80、CapsuleCornerRadius 20/0–48 | 拖动 MotionDuration 480→910 **落盘 settings.json（两次）** |
| 2 | 标题栏月亮切换深色后 island 侧 SettingsTheme 不更新 | toggleDark 发送 `map[string]any{"__pick":…}`，config.rs 只收 "System/Light/Dark" 字符串 | root.go 改发显式 "Dark"/"Light" | dk3/dk4：深色全窗生效且来回切换 |
| 3 | 开发树 e2e 从来没有岛：spawnIsland 静默失败 | 只在 Center.exe 同目录找 LyricIsland.exe（发布态布局），Go\\settings-center 下不存在 | island.go 加 DOTISLE_MODULE_DIR 回退（发布态兄弟目录仍首选） | 岛 BUILD 日志出现，stateprobe 全过 |
| 4 | 全部动效/弹簧滑条显示默认值而非岛真实值 | islandSetting 平面查找读不到嵌套 `Motion`/`Motion.Spring`（settings.json 形状）；写入路径本身正确（validate_setting 前缀重映射） | islandSetting 镜像 model.rs setting() 的前缀反解 | trace：`dur=850`（岛存储值）而非 def 480 |
| 5 | packet 到达后 UI 冻结在旧值（滑条拖完显示值不变） | ApplyPacket 只赋值不重绘，mygo 仅在输入事件时重建 | ApplyPacket 末尾 Refresh()（Invalidate） | 拖动→提交→快照回流后值稳定更新 |
| 6 | 瞬时管道失败让整个 UI 闪回默认值 | worker 失败包 `State{Err}` → main.go 把 a.State 覆盖为 nil | ApplyPacket(nil) 保留最后一份好状态 | run P trace 全程 conn=true err="" |
| 7 | 滑条停顿后松手/PostMessage 拖动不提交（拖完回弹） | 释放提交要求 `frac != prev`，最后一步移动已被中间帧读过 | kit.go held 标志：按下置位、松手必提交；键盘分支加 Focused() 门控（防自动化误提交落盘） | MotionDuration=910 落盘 |
| 8 | 深色模式页头浅字浅底不可读 | root 根底色固定 `RGBA(245,245,247,1)`（§5.10 DWM 修复引入）不跟随主题 | 改用 `Col(s.Surface)` | dk3：整窗深色、标题清晰 |
| 9 | 待办弹窗键盘流缺一步完成路径 | — | 标题框 Submitted()→sendTodoSave | 输入+Enter：todos.json 落盘"测试四"、弹窗关闭、列表实时 2 件 |
| 10 | pages_home.go gofmt 不净；e2e 脚本 wait 解析在 PS5.1 下崩 | — | gofmt -w；`([int]$t.Substring(5))` | build.cmd 每轮全绿 |

### 10.2 新增验证证据（交接包 §6 之外）

- 滑条拖动闭环：`480.0 → 909.8 ms`（镜像标签），松手提交后 settings.json `Motion.Duration: 910.0`，离开页面返回后保持——拖动预览、松手提交、island 校验入库、快照回流四段全部真机验证。
- 待办全流程：新建弹窗打开 → 中文输入（SendKeys 直打 CJK 可用）→ Enter 保存 → todos.json 落盘 → 列表实时刷新。管道层另有 stateprobe（cmd/stateprobe，新增）验证 todoSave 应答与 todos 计数。
- 深色模式：标题栏月亮切换（修复后），深浅来回各验证；通知页浅/深两态渲染正常。
- 字体选择器：入口（岛尺寸页·更改字体）→ 弹窗（注册表枚举、搜索框、预览面板、未选中时确认按钮禁用态）→ 遮罩点击关闭。
- 开屏动画：DOTISLE_SIG_SLOW 钩子（signature.go，env 门控）拉长揭示后逐帧截取，字形/笔迹揭示正确（sq2："Do" 大写完成、小写笔迹推进中）。此前 sp 轮的空白是钩子首版缩放不对称（duration 拉长但 sigInk 时钟未同步）的假象，已修。
- Ctrl+K 调色板：面板、页面跳转列表、背景遮罩均正常。
- tab132 播放器页：响应范围分段、浏览器媒体开关渲染正常（交接包中断点第一张截图的核对结论）。

### 10.3 已知限制与未决（诚实清单补充）

1. **聚焦输入框吞掉下一次外部按下（mygo v0.9.0 行为）**：弹窗内 TextInput 聚焦时，点按钮的第一下不触发（无错误、无动作），第二下正常。已在待办弹窗加 Enter 提交缓解；鼠标两击是框架层行为，未在 Go 层修复（投递前消费，无法拦截）。Rust 原版单击即生效，此处为已知回归。
2. **测试数据污染**：dotisle-verify 的 SpringMass 在某轮自动化交互中被写成 1.0（滑条 frac 被推到 ≈0.507 后被当时的新键盘提交分支落盘）。已加 Focused() 门控堵住该类写入；隔离测试目录，无产品影响。若复现，查 mygo pressedPrimary 生命周期。
3. **壁纸与文件对话框**：GetOpenFileNameW 真模态会阻塞 UI 线程，自动化流程跳过（沿用 §8 记录），交付前需一次人工验证。
4. **诊断残留**：ApplyPacket 的 packet trace（DOTISLE_TRACE_UI 门控）、traceEvent 桩、cmd/stateprobe 探针保留，交付打包时可删可留（均 env/独立目录门控，无运行时开销）。

### 10.4 e2e 环境要点（下个 AI 必读）

- **本机 PowerShell 工具环境缺 `%LocalAppData%`** → go build/vet/test 全部因 GOCACHE 缺失失败且 cmd 的 `%errorlevel%` 回显是解析期展开不可信。用 `Go\settings-center\build.cmd`（显式设 GOCACHE/GOPATH，含 gofmt/vet/build/test 四步）。
- 嵌套 `powershell -File` 会掐断本会话宿主（exit 4294967295 无输出）；用 `& script.ps1` 会话内调用。
- e2e-interact.ps1 新参数：`-Trace`（DOTISLE_TRACE_UI）、`-SigSlow <倍数>`（开屏动画慢放）、`-EarlyShots <前缀>`。
- 坐标不再手推：先 shot 再按截图实测（hub 卡片中心 x≈310/876、行 y≈301/404/502；分组条目 top≈225+100k；滚轮每格 ≈87.5 DIP）。

## 10. 审查产出建议

审查者可直接按第 7 节顺序产出发现项，标注 文件:行号 与 严重级别。以下三类最有价值：
① 会破坏"变更不重放"或管道复用语义的缺陷；② payload 与 Rust 校验规则不一致导致
静默失败的键值；③ ABI/unsafe 边界的内存安全问题。

## 11. 第三轮接力（2026-11-14 夜，交接包 v3）

### 11.1 修复（5 项，全部真机/单测复验）

1. **hoverLayer 替换底色**（kit.go + 21 调用点）：mygo Background 为替换语义，悬停状态层曾擦掉实心填充致白字浅底"按钮消失"。改 `hoverLayer(e, base, fg, enabled)` + `overCol` source-over 叠加。单测 TestHoverKeepsButtonFill。
2. **leafSettingsSlider 释放丢提交**：held 标志 + Focused 门控同步 kit.Slider（v2 只修了 kit.Slider，leaf 漏同步）。壁纸三滑条拖拽落盘恢复。
3. **verify.rs pure/waiting 期望掩蔽 + main.rs 漏 `mod timeline;`**：current_lyric 空行回退标题掩蔽分支断言（CI --verify 必挂潜雷）；mod 列表漂移致 cdylib 编不过。注意 launcher+cdylib 结构：重编设置侧必须 `--lib`。
4. **键盘小范围滑条卡死**（审查工作流发现）：键盘提交后回绕 off-grid frac 与 mygo 0.01 网格打架，range<~50 全范围卡死。键盘分支不回绕 + 同值去重。单测 TestLeafSliderKeyboardSmallRange。
5. **轨外拖拽提前释放+逐帧提交洪泛**（审查工作流发现，kit.Slider 同源）：Pressed() 包含判定 vs 捕获按压。改 PressedButton() 捕获感知。单测 TestLeafSliderOutTrackDragCommitsOnce + 真机 wp14。

### 11.2 验证证据

- 壁纸/文件对话框/文件夹选择器全闭环：e2e-wallpaper.ps1（模态 #32770 Win32 自动化），截图 %TEMP%\wallpaper-shots（wp1-wp14、fd1-fd3、dlg-*）。
- 打包：preflight 46/46；package_release r4 exit 0；runtime Center+15 DLL 门禁；打包态冒烟 pk1-pk4；verify_packaged_ui 四报告 success（validation-local-r4）；Rust workspace 518 passed/0 failed；Go 15 单测；go test ./...（1.27.3+GOTOOLCHAIN=local）全 ok；verify_release_source success；netease 9/9。
- 审查工作流 wf-f33c9f77-112（8 代理）：2 条 major 确认并已修（11.1.4/5），其余维度无确认缺陷。
- 哈希防护：补建/重编前后 dotisle_core/dotisle_themes/LyricIsland.exe sha256 不变。

### 11.3 限制与未决

- CI 真实 runner 未跑（推送需用户批准）；本地等价全绿 + Go pin 实测一致。
- moduleStage DLL 选择对话框未单测（与壁纸同款 openFileDialog，机制已证）。
- mygo 聚焦输入两击限制仍在（Enter 缓解）。
- 中间包 publish/local-5.0.0、-r2、-r3 勿发；**终包 = local-5.0.0-r4**。


## 12. 重新审查与修复（2026-11-14，r5）

本节取代旧 §11.3 中“终包 r4”和“必须 fork mygo 修输入两击”等已过时结论。现有 FieldOutline 与 TestFocusedFieldKeepsNextPress 已修复焦点导致的元素身份变化，不应继续把它归咎于框架。

| 级别 | 确认缺陷 | 修复 |
|---|---|---|
| P1 | Go 读 backend 的 track/frame/positionSeconds，但 settings 管道发布 title/artist/position/duration 和 musicPreview.lyricFrame，歌曲/歌词/进度及播放按钮一直空态 | 对齐全部音乐卡片、预览和控件的真实协议字段，增加真实形状回归测试 |
| P1 | 关闭按钮只 Hide，pendingPage 不 Show；lastPending 遇空值不清理，同一目标页仅首轮有效 | 唤起时 Restore/Show，收到 pageSeen 后的空值清理去重标记 |
| P1 | moduleApply 传入当前 Center PID，更新器等待它退出，Center 却不退出 | 仅收到 success:true 后 Close；拒绝或超时保留窗口并显示原因 |
| P1 | worker 在旧主程序死亡后 return，控制中心无法连接重启后的新 PID | 保留轮询并报告断连，待新岛恢复快照 |
| P2 | 通用命令和 moduleStage 忽略业务失败，timerCreate 失败没有可见反馈 | 统一处理失败回执并在 UI 线程显示 Toast |
| P2 | todosPage 和根 dialogViews 各绘制一次待办弹窗 | 只保留根弹层入口，离屏断言标题仅渲染一次 |
| P2 | 减少动态效果开关把原值发回 Animation | 提交反值，保持本地 reducedMotion 与原生 Animation 一致 |
| P2 | 首次偏好保存未建目录、读写错误被吞；固定 .tmp 与其他进程冲突 | 区分缺失与读取失败、创建目录、唯一临时文件+同步+重命名、显示保存失败 |
| P2 | 管道只有回复读取有超时；连接错误靠英文子串判断 | 同一 deadline 覆盖读写，以 Win32 错误码判定可重试连接；新增大请求静默服务器写超时回归 |
| P1 | build.cmd 记录错误后继续，未运行 island/prefs 测试；打包可选 Center 且易用旧二进制 | 构建检查失败即停、全包测试；打包强制 Center，发现源构建与输入哈希不同则拒绝 |
| P2 | 布局报告仅依赖外部环境开启；安装升级进程过滤漏 Center | 验收入口显式开启并核验布局报告，安装过滤增加 LyricIsland.Center.exe |

验证：Go 35 项 / Rust 518 项通过；preflight 46/46；原生五报告通过，布局 2740 views。隔离管道内完整验证隐藏/重复唤起/最小化恢复、主程序重启后的快照恢复、失败 Toast，以及模块 stage→cancel→stage→apply。最后一步 Center 自行退出，更新器 journal 为 completed / success:true，新岛启动确认通过。详见 HANDOFF 顶部与本次 outputs/verification。

图像为真实快照的 MyGo 软件离屏渲染，已查看 idle/checking/staged 三态；没有把它们冒充桌面截屏。验收使用额外 QA 编译标记，未把测试入口带进交付 Center。真实文件选择器与安装向导本轮未桌面点击复验；CI 未推送执行。
