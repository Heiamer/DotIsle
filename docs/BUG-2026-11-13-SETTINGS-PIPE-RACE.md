# 3.5.2 设置窗口与主程序命名管道竞态（撤回原因）

- 日期：2026-11-13
- 影响版本：3.5.2（已撤回）
- 状态：已按第 6 节建议修复（v5.0.0 symphony 开发分支），服务端改为复用管道实例、客户端只读退避重试、设置端先查单实例互斥；并发回归测试见 `Rust/crates/lyric-backend/src/ipc.rs` `concurrent_state_requests_never_hit_a_doomed_instance`
- 撤回结果：GitHub / Gitee 的 3.5.2 发布与标签均已删除，`main` 回到 `ee20c1a`（3.5.1），本机恢复 3.5.1

## 1. 现象

1. 点击「设置」时偶发弹出单实例提示：
   `已有点屿（版本未知）正在运行。你正在启动的是此目录的 3.5.2……请先从托盘退出正在运行的点屿，再启动这一份。`
2. 设置窗口保存时偶发失败：
   `保存失败：管道的另一端上无任何进程。(0x800700E9)。请重试。`
3. 主程序本身是正常的：用同一时刻查询 `LyricIsland.Native.Settings.v1` 的 `state`，能返回
   `version=3.5.2`、`processId`、`moduleReady=true`；问题只出现在设置进程与主程序之间的请求。

`0x800700E9` 即 `ERROR_PIPE_NOT_CONNECTED (233)`，表示客户端已拿到管道句柄，但服务端在请求完成前断开了连接。

## 2. 现场证据（2026-11-13 19:05–19:17）

- 用户实例：`LyricIsland.exe` PID 41732，`Portable.json` 为 3.5.2；`LyricIsland.exe`、`LyricIsland.Settings.exe`、`dotisle_core.dll`、`dotisle_settings.dll` 与 3.5.2 发布包逐字节一致。
- 主程序管道存在且可应答：直接向 `\\.\pipe\LyricIsland.Native.Settings.v1` 发送 `state`，返回 `version=3.5.2`、`processId=41732`、`moduleReady=True`。
- 手动启动同目录 `LyricIsland.Settings.exe`，同一秒出现第二个 `LyricIsland.exe`（PID 44292），窗口标题为 `点屿 · DotIsle 3.5.2`，内容正是上面的单实例提示；该进程查询运行实例版本失败，因此显示「（版本未知）」。
- 设置窗口（教程第 04/04 页）保存时出现 `0x800700E9`。
- 原生日志 `Data/Logs/` 中没有 `IPC_DROP` 记录。该日志只在服务端**已接受连接**后、请求读取/身份校验/回调阶段失败时写入；没有记录说明失败发生在服务端接受请求之前（连接阶段），而不是被身份校验或回调拒绝。

## 3. 涉及代码（提交 ffd7e9c）

| 文件 | 位置 | 作用 |
| --- | --- | --- |
| `Rust/crates/lyric-backend/src/ipc.rs` | 16、48、51 行 | 命名管道服务端：单实例、单请求循环，写回复后 `FlushFileBuffers` → `DisconnectNamedPipe` → `CloseHandle`，再重新创建实例 |
| `Rust/crates/lyric-backend/src/io.rs` | 21–43 行 | `pipe_request`：连接重试上限 2 秒，只重试错误码 2 / 231 / 535；写入或读取失败立即返回，且有意不重放变更请求 |
| `Rust/crates/dotisle-app/src/remote.rs` | 26–40 行 | `Remote::new`：第一次 `state` 失败即视为「主程序未运行」，启动 `LyricIsland.exe --from-settings --data-dir …` |
| `Rust/crates/lyric-native/src/main.rs` | 5418、5435 行 | 单实例互斥 `Local.LyricIsland.Native.Vello.v3`；冲突时固定从正常管道查询版本，失败则显示「版本未知」，然后弹窗 |

## 4. 根因分析（高置信度假设，撤回前未加服务端日志证实）

服务端的生命周期如下：

1. 创建单实例管道（`nMaxInstances=1`），接受一个客户端；
2. 读取一个请求、调用回调、`WriteFile` 写回复；
3. `FlushFileBuffers` 等客户端读完；
4. `DisconnectNamedPipe(pipe)`；
5. `CloseHandle(pipe)`；
6. 回到第 1 步，重新 `CreateNamedPipeW`。

在第 4 步与第 5 步之间，管道名仍然存在且处于「已断开」状态。此时新客户端 `CreateFile` 会**成功连接到一个即将被关闭的实例**；随后第 5 步关闭句柄，客户端的写入/读取就会得到 `ERROR_PIPE_NOT_CONNECTED (233)`。客户端把该错误当作致命错误；`Remote::new` 又把一次 `state` 失败当作「主程序没在运行」，于是启动了第二个主程序，触发单实例弹窗。第二个进程在查询版本时也可能撞上同一窗口，因此显示「版本未知」。

设置进程启动时会连续发多个请求（初始 `state`、教程状态、健康检查等），并发度比日常轮询高，命中该窗口的概率也随之升高。3.5.1 使用同一套管道代码，但设置窗口的启动请求序列不同；3.5.2 上该现象被稳定触发（本次现场手动启动设置进程即可复现），因此作为撤回 3.5.2 的直接原因。跨版本触发率未做统计，不作定量结论。

## 5. 复现步骤

1. 启动 `LyricIsland.exe`（正常实例，非测试管道）；
2. 运行同目录 `LyricIsland.Settings.exe`；
3. 观察是否出现第二个 `LyricIsland.exe` 进程和单实例弹窗；在设置中保存教程状态或参数，观察是否报 `0x800700E9`。

本次现场为可复现；隔离环境中的并发压测脚本未在撤回前完成最终收敛，修复后请以第 7 节的验证结果为准。

## 6. 修复建议

1. **服务端（首选）**：不要每处理一个请求就关闭管道实例。保持同一个实例：`DisconnectNamedPipe` 后直接 `ConnectNamedPipe` 等待下一个客户端；或预创建 ≥2 个实例（`nMaxInstances` ≥ 2），避免出现「可连接但即将销毁」的实例。
2. **客户端**：对只读命令（`state`）在 233 / 109 时做短暂退避重试；对变更请求，仅在**写入失败（请求未完整送达）**时重试，写入成功后读取失败的请求不要重放，保持现有「never replay a mutation」原则。
3. **设置端**：首次 `state` 失败时先检查单实例互斥 `Local.LyricIsland.Native.Vello.v3` 是否存在；存在则重试管道若干次，不要启动新的主程序。
4. **单实例弹窗**：版本查询使用与服务端一致的管道名（包含 `LYRICISLAND_TEST_PIPE`），避免测试实例或竞态时误报「版本未知」。

## 7. 验证计划

- 并发压测：多线程 × 多次 `state` 请求，统计 233 / 超时次数应为 0；
- 反复打开设置 50 次：不得出现第二个 `LyricIsland.exe` 进程，不得弹单实例提示；
- 教程与参数保存各 100 次：不得出现 `0x800700E9`；
- 加入 CI：设置启动冒烟 + 管道并发回归，至少在两个并发客户端下保持稳定。

## 8. 撤回记录（2026-11-13）

- GitHub：`v3.5.2` Release 与 tag 已删除，`releases/latest` 回到 `v3.5.1`；`main` 强制退回 `ee20c1a`，main 上的 `Assets/UpdateManifest.json` 为 `v3.5.1`。
- Gitee：`v3.5.2` 发行版已删除，`releases/latest` 回到 `v3.2.0`，标签列表无 `v3.5.2`。
- 本机：用官方 3.5.1 便携包覆盖安装目录（保留 `Data`），删除 3.5.2 的 `dotisle_*.dll`；当前运行 3.5.1。
- 源码：保留在 `codex/core-dll-modules-ci-20261113` 分支与本地源码包 `DotIsle-3.5.2-Source.zip`。
