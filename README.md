<div align="center">

<img src="Assets/LyricIsland-v3.png" width="88" alt="点屿 DotIsle">

# 点屿 · DotIsle

**把音乐、歌词、专注与提醒，放进 Windows 桌面的一座小岛。**

音乐控制与实时歌词 · 番茄钟与计时 · 待办提醒 · 通知与设备

[下载安装版](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Setup.exe) · [下载便携版](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Portable.zip) · [3.2.1 更新说明](https://github.com/Heiamer/DotIsle/releases/tag/v3.2.1) · [反馈问题](https://github.com/Heiamer/DotIsle/issues)

Windows 10 / 11 · x64 · Rust 原生应用

</div>

## 五套主题，同一套功能

![点屿 3.2.1 五套主题：默认、Nothing、云瓷、终端 TTY 与像素 Pixel](docs/images/themes-3.2.1.png)

截图来自应用原生渲染；歌曲、歌词与待办为演示数据。点击图片可查看大图。

| 主题 | 风格 |
| --- | --- |
| **默认** | 保留原有配色、布局、图标和窗口行为。 |
| **Nothing** | 黑白工业风、简洁线条图标与点阵计时数字。 |
| **云瓷 · Porcelain** | 暖灰、瓷白、亮蓝，圆润卡片与白色滑钮。 |
| **终端 · TTY** | 等宽字符、方正面板、色块唱片和离散进度。 |
| **像素 · Pixel** | 中英像素字体、16 位机卡带、方块图标与阶梯边框。 |

在 **控制中心 → 设置 → 外观** 选择主题。五个选项块支持点击、拖动和键盘切换，重启后保留选择；四套非默认主题使用固定窗口尺寸。

### 用自己的图片做背景

在同一页面的 **自定义图片背景** 中选择本机 PNG、JPEG 或 BMP，可分别调整图片透明度、明暗、磨砂度，以及填满 / 适应方式。

每套主题单独保存图片和参数。导入图片会复制到本机数据目录，不上传服务器；关闭背景时仍保留配置，也可恢复默认。背景功能默认关闭。

### 更柔和的展开与收起

<p align="center"><img src="docs/images/motion-3.2.1.webp" width="640" alt="灵动岛展开、收起与中途反向的原生动画预览"></p>

展开、收起时加入轻微高斯模糊，静止后恢复清晰；中途反向也能连续过渡。开启“减少动态效果”后关闭过渡模糊。

## 下载与开始使用

最新正式版：**[3.2.1](https://github.com/Heiamer/DotIsle/releases/tag/v3.2.1)**。发行包由 [GitHub Actions](https://github.com/Heiamer/DotIsle/actions/runs/42060627924) 编译，运行时不需要安装 Rust、Python 或 .NET。

| 文件 | 用途 |
| --- | --- |
| [安装版 · Setup.exe](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Setup.exe) | 推荐使用。标准安装向导，支持开始菜单与卸载。 |
| [便携版 · Portable.zip](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Portable.zip) | 免安装，完整解压后运行 `LyricIsland.exe`。 |
| [完整源码 · Source.zip](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Source.zip) | 对应发行构建的源码、资源与第三方许可。 |
| [SHA256 校验清单](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-SHA256.txt) | 校验下载文件是否完整。 |
| [验证记录 · Validation.zip](https://github.com/Heiamer/DotIsle/releases/download/v3.2.1/DotIsle-3.2.1-Validation.zip) | Actions 中的控制中心与主题渲染验收记录。 |

1. 安装，或把便携包**完整解压到可写目录**；不要在压缩包内直接启动 EXE。
2. 运行点屿，从系统托盘打开控制中心。
3. 打开播放器并播放音乐，点屿会尝试连接媒体会话、读取歌曲并匹配歌词。
4. 在控制中心调整灵动岛排版、主题、图片背景与动画；也可直接使用计时和待办。

请保留程序旁的资源、字体、更新助手和许可证。关闭控制中心后，灵动岛继续运行；完全退出主程序后，不再提供计时和待办提醒。

### 从旧版更新

在“关于与更新”检查更新，或手动下载新包。手动迁移时先退出旧版，备份并复制原运行目录的 `Data` 到新目录；不要同时运行两个版本操作同一份数据。

发行页中的 `LyricIsland-3.2.1-Portable.zip` 与对应 SHA256 文件供旧更新器识别，日常下载请使用上表的 DotIsle 包。国内镜像可能晚于 GitHub 更新。

## 能做什么

- **音乐与歌词**：播放 / 暂停、切歌、进度调节、原文与翻译、多种展开排版、封面动态配色。实际控制能力取决于播放器接口。
- **专注与计时**：番茄钟、倒计时、秒表和流程计时，支持多项目与暂停恢复。
- **待办与提醒**：记录、编辑、完成、撤销，以及日期和时间提醒。
- **通知与设备**：在岛体中展示通知与设备连接状态，具体能力受系统、权限及设备接口影响。
- **外观与交互**：五套主题、独立图片背景、明暗模式、灵动岛布局与动画设置。

## 3.2.1 修了什么

- 修复 Nothing 等非默认主题仍能拖边改变窗口大小的问题。
- 主题切换改为五个选项块；修正同色滑轨、空待办实心球、歌词拥挤与计时字体不一致。
- 移除 Nothing 品牌图标旁的装饰红点，并为新主题绘制匹配图标。
- 优化网易云连接探测和连续拖动的请求合并，优先处理最新目标。
- 修复图片图集扩容时的上传顺序，避免封面等图片出现黑块。

**网易云兼容范围：** 本机当前客户端已验证连接、十次进度跳转及快速连续跳转，验收中未记录到任务栏注意力请求；暂停态尚未通过验收，不同客户端版本、最小化和托盘场景仍需分别验证。详见[本版发布说明](https://github.com/Heiamer/DotIsle/releases/tag/v3.2.1)，反馈时请带上客户端版本和复现步骤。

## 源码与构建

使用 **Rust、Win32、Vello Hybrid、wgpu 与 Parley**。源码包含发行所需字体、资源和第三方补丁，不包含用户数据或编译缓存。

推荐使用仓库的 [Windows Release 工作流](.github/workflows/windows-release.yml) 构建；本地环境与命令见 **[3.2.1 构建说明](BUILD-3.2.1.md)**。本版通过 490 项工作区测试，以及主题、背景和控制中心渲染检查；自动检查不能替代所有播放器与桌面交互场景的实机验证。

## 许可与反馈

项目许可见 [LICENSE](LICENSE)。第三方与字体许可见 [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md)、[FONT-NOTICE.md](FONT-NOTICE.md) 和 [licenses](licenses/)。像素字体使用 Fusion Pixel，随包保留其许可。

欢迎在 [Issues](https://github.com/Heiamer/DotIsle/issues) 提交问题或建议。请附 Windows 版本、点屿版本、播放器版本、复现步骤和相关截图；分享日志前请移除个人信息。
