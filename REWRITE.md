# 歌词与进度实现说明

版本：3.2.5。作者：Heiamer。

## 当前实现

- lyric-core 使用 Track、Cue、Word、Lyrics 单时间轴模型，负责身份匹配、解析、缓存、本地歌词检索和同步。
- lyric-backend 负责六个在线歌词源、系统媒体会话和网易云自动原生进度。新进度实现只读进程内存，通过客户端自身类型信息定位对象，仍包含经指定客户端 SHA256 验证的字段偏移与指令布局；未恢复此前移除的项目实现或特征码。
- 原生界面使用连续弹簧求解与 Hermite 中断衔接。控制中心、六套主题、计时、待办、通知与更新继续使用同一套原生界面。

## 依赖

QQ 歌词块解码器采用 SuJiKiNen/LyricDecoder 的 MIT 代码，提交 36fc1eaf31f8ef14a23de6f9f1427ce04178a900；保留文件头和完整许可。两个无符号移位修正见 Rust/vendor/qq-lyric-codec/SOURCE.md。

网易云 EAPI 请求封装参考 NeteaseCloudMusicApiEnhanced/api-enhanced，提交 4a7d557326d16d2f1e5172ee69c9471eb3a50173，MIT，Binaryify。未捆绑其 Node 服务端。

XML 由 roxmltree 0.20.0 解析。上述依赖的许可和其他界面、字体、图标依赖分别见 THIRD-PARTY-NOTICES.md、FONT-NOTICE.md、licenses/ 和 Rust/vendor/。根目录 LICENSE 适用于项目自有代码，署名 Heiamer。

## 缓存与控制

新缓存为 database/dotisle-payload-v3.json 和 database/dotisle-lyrics-v3.json，设有大小、数量和有效期限制。旧歌词缓存不再读取；配置与 song_maps.db 的可选只读导入继续保留。

逐字歌词只使用真实词时间；普通 LRC 或纯文本不生成虚假的词时间。高级 TTML 方言的支持范围由解析器决定。

网易云正常启动即可自动连接；本机页面通道只作为可选回退。原理与适配范围见 NETEASE-SEEKFIX.md。缺少有效播放实例或新鲜样本时不提交跳转；同曲显示最多容忍 2 秒采样断档，控制和确认仍需要 800 毫秒内的新样本。

## 验证

本次普通启动的真实播放器验收见 docs/verification/3.2.5-live.json；可见后台窗口、最小化和隐藏托盘分别检查。构建和源码版本、根许可及依赖许可由发布工作流独立验证，见 BUILD-3.2.5.md。
