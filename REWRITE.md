# 歌词与进度实现说明

版本：3.2.4。作者：Heiamer。

## 当前实现

- lyric-core 使用 Track、Cue、Word、Lyrics 单时间轴模型，负责身份匹配、解析、缓存、本地歌词检索和同步。
- lyric-backend 负责六个在线歌词源、系统媒体会话和网易云本机页面通道。网易云进度不读取进程内存，不使用特征码或地址偏移。
- 原生界面使用连续弹簧求解与 Hermite 中断衔接。控制中心、六套主题、计时、待办、通知与更新继续使用同一套原生界面。

## 依赖

QQ 歌词块解码器采用 SuJiKiNen/LyricDecoder 的 MIT 代码，提交 36fc1eaf31f8ef14a23de6f9f1427ce04178a900；保留文件头和完整许可。两个无符号移位修正见 Rust/vendor/qq-lyric-codec/SOURCE.md。

网易云 EAPI 请求封装参考 NeteaseCloudMusicApiEnhanced/api-enhanced，提交 4a7d557326d16d2f1e5172ee69c9471eb3a50173，MIT，Binaryify。未捆绑其 Node 服务端。

XML 由 roxmltree 0.20.0 解析。上述依赖的许可和其他界面、字体、图标依赖分别见 THIRD-PARTY-NOTICES.md、FONT-NOTICE.md、licenses/ 和 Rust/vendor/。根目录 LICENSE 适用于项目自有代码，署名 Heiamer。

## 缓存与控制

新缓存为 database/dotisle-payload-v3.json 和 database/dotisle-lyrics-v3.json，设有大小、数量和有效期限制。旧歌词缓存不再读取；配置与 song_maps.db 的可选只读导入继续保留。

逐字歌词只使用真实词时间；普通 LRC 或纯文本不生成虚假的词时间。高级 TTML 方言的支持范围由解析器决定。

网易云页面通道通过随包 Start-NetEase-SilentControl.cmd 启动，具体步骤见 NETEASE-SEEKFIX.md。缺少有效播放实例或新鲜样本时不提交跳转；同曲显示最多容忍 2 秒采样断档，控制和确认仍需要 800 毫秒内的新样本。

## 验证

本机网易云 3.1.43.205627 的前台、最小化与隐藏托盘场景已通过连续跳转、快速请求合并、暂停稳定、恢复播放、播放中跳转、在线歌词和原状态恢复检查。该结果仅覆盖已测试的客户端和歌曲，汇总见 docs/verification/3.2.4-live.json。

GitHub Actions 对提交源码重新执行版本与许可检查、393 项工作区测试、9 项页面读取检查、发行编译、打包和四项界面验收。源码与安装文件的版本一致性由工作流检查，构建说明见 BUILD-3.2.4.md。
