# 统一控制中心

5.0 后续修复将旧设置应用 `Rust/crates/dotisle-app` 删除。新包不再构建、启动或携带 `LyricIsland.Settings.exe` 和 `dotisle_settings.dll`。

## 进程与状态

- `LyricIsland.exe` / `dotisle_core.dll`：灵动岛、后台服务、设置和业务数据的唯一写入方。
- `LyricIsland.Center.exe`：Go + MyGo / Direct3D11，所有可见设置、概览和编辑器的唯一入口。
- `dotisle_desktop.dll`：从旧应用提取的桌面组件渲染器。通过 C ABI 嵌入 Center 进程，只有组件窗口，不含设置窗口或设置导航。
- `LyricIsland.Updater.exe`：独立的更新事务和失败回滚。

没有桌面组件时不启动桌面渲染线程。存在组件时，主程序可用 `Center.exe --warm` 隐藏启动中心；打开控制中心会唤起同一个进程。关闭中心窗口隐藏窗口，组件继续显示；完全退出主程序会结束其中心进程。

状态泵确认已连接的主程序退出后，控制中心主动结束桌面渲染线程并退出，包括主程序异常终止的情况，避免留下无法响应移除的孤立组件。短暂管道故障不视为进程退出；重新启动主程序后从已保存的位置和配置恢复组件。

Go 的现有状态泵取得原生快照后，将快照同步复制给 DLL。DLL 合并未消费的快照，不增加另一条状态轮询。播放时只有音乐组件持续请求帧，其余组件按状态更新或每秒刷新。组件操作通过后台命令工作线程发回主程序，DLL 不写设置或数据文件。

## C ABI 1

所有指针和缓冲区归调用者所有，DLL 在返回前复制 JSON。输入上限为 8 MiB。Go 调用阻塞的 `run` 时固定 OS 线程，运行期间保持 DLL 加载。

| 导出 | 用途 |
| --- | --- |
| `dotisle_desktop_abi()` | 返回接口版本 1 |
| `dotisle_desktop_run(data,len)` | 在专用 UI 线程运行组件事件循环；一个进程生命周期只启动一次 |
| `dotisle_desktop_snapshot(json,len)` | 提交最新业务快照 |
| `dotisle_desktop_running()` | 查询运行状态 |
| `dotisle_desktop_stop()` | 请求退出并唤醒事件循环 |
| `dotisle_desktop_layout(json,len,out,capacity)` | 共用 `lyric-core::custom_layout` 的校验与原生几何；null 返回推荐布局 |
| `dotisle_desktop_verify(path,len)` | 用真实桌面渲染器离屏绘制全部类型、尺寸与明暗主题 |

`layout` 返回 UTF-8 JSON 的字节数，容量不足时返回所需字节数的负值并保持输出缓冲区不变。校验错误以 JSON `error` 返回。其他操作返回 0 表示成功，非 0 为失败。界面不将 DLL 运行退出当成仍在运行。

## 本地验收

```powershell
python build/verify_packaged_ui.py --runtime <完整运行包> --output <验收目录>
```

验收直接运行包内 `Center.exe --verify`，覆盖新控制中心路由及布局编辑器、桌面组件全部类型的三种尺寸和两个主题；随后启动隔离原生实例，验证组件添加、尺寸变化、删除、单个宿主、布局保存及重启恢复。日志与截图保存在输出目录，不能拿旧设置截图代替。

`Center.exe --module-health <目录>` 在单实例检查前运行，无窗口、无业务写入，检查桌面布局 ABI 和主题 ABI。模块更新器在替换后使用此入口，避免依赖已删除的旧程序。

## 升级边界

安装器保留识别旧设置进程的清理逻辑，用于升级和卸载旧安装；这是对旧版本的清理，不是保留旧应用。`repair-updater.yml` 是 3.5.x 专用修复流程，不适用于本结构。旧 3.5.x 更新助手可能硬校验旧文件集合，跨版本 OTA 必须单独验收；未通过前使用新完整安装包升级。
