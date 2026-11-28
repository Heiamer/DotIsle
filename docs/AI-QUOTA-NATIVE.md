# 原生 AI 额度采集（工作副本）

适配基线：BrimDeck `d3215210e2f337c93784e862bea9830754030819` 的 `ProviderCatalog.Sources`。39 个额度来源登记在控制中心；不安装 .NET，不运行桥接 EXE。Pi 是上游用量统计项，不是额度来源；Antigravity 按要求排除。

## 原生读取路径（39 个，不含 Antigravity）

| 来源 | 读取途径 | 原生显示 |
| --- | --- | --- |
| Codex | 本机会话缓存，沿用既有适配 | 5h、周与识别到的其它时间窗 |
| Claude | Claude Desktop 本机缓存，沿用既有适配 | 5h、周 |
| Cursor | 当前 Windows 用户的 Cursor 本地登录 | 当前账期与接口返回的用量，不把账期当周额 |
| DeepSeek Harness | 添加后读取本机 DSH 登录；没有账户授权时用 DSH 保存的 DeepSeek API 密钥 | 余额、赠送余额、消费 |
| ZCode | 添加后读取当前用户已登录凭据；通过 Windows CNG 验证解密 | Coding Plan、Start Plan（剩余 / 总量 TOKEN）、MCP |
| DeepSeek | 手动 API 密钥 | 人民币／美元余额 |
| 智谱 GLM | 手动密钥 | 套餐周期与工具调用 |
| Z.ai GLM | 手动密钥 | 套餐周期与工具调用 |
| New API | 自填站点与密钥 | 密钥余额、累计消费、可确认的账户余额；未确认单位则显示原始配额 |
| Sub2API | 自填站点与密钥 | 密钥窗口、订阅窗口或钱包余额 |
| OpenRouter | 手动密钥 | 预算余额、消费；接口授权时显示账户余额 |
| Command Code | 本机 Command Code `auth.json`，固定官方域 | 5h、周、月及可用积分 |
| Command Code API | 手动 Studio 密钥 | 同上 |
| OpenCode | 本机 OpenCode `auth.json` 的 Go／Console API key；数据库 OAuth 暂不支持 | Go 的 5h、周和月 |
| OpenCode Go | 手动 Console 密钥 | 同上 |
| Kimi Code API | 手动 Kimi Code 密钥，支持国内和国际区 | 5h、周、月及可识别的加油包 |
| Kimi 开放平台 | 手动开放平台密钥，支持国内和国际区 | 账户余额、现金和代金券 |
| Xiaomi MiMo API | 手动开放平台密钥 | 余额、本月扣费（接口可用时） |
| Xiaomi MiMo Token Plan | 手动 Token Plan 密钥 | 套餐积分、重置时间 |
| Xiaomi MiMo 已关联套餐 | MiMo Code 的 `auth.json`；只读取绑定到对应官方站点的已知来源 | Token Plan 积分及开放平台余额 |
| Kimi Code 本机 OAuth | 已登录的本机 Kimi Code；固定官方域，不自动续期 | 5h、周、月及可识别的加油包 |
| Grok Bot | 当前 Windows 用户已登录的本机数据；只读额度查询 | 上游返回的额度窗口与剩余量 |
| CodeAgent | 当前 Windows 用户已登录的本机配置；不唤起 CLI | 套餐额度及周期 |
| WorkBuddy | Go 控制中心发起国内官网授权 | 个人套餐与额度包；未实现自动续期或额度包日期，授权入口仍需实号验证 |
| WorkBuddy AI | Go 控制中心发起国际官网授权 | 国际个人套餐；同上 |
| Xiaomi MiMo 桌面订阅 | 在隔离的浏览器窗口登录官网 | 桌面订阅的周额度与重置时间 |
| Qoder | 当前用户的国际版桌面登录 | 当前套餐积分；不含 CLI／IDE 备选登录 |
| Qoder CN | 当前用户的国内版桌面登录 | 同上，区域独立 |
| 千问办公 | 当前用户的国内版桌面登录 | 个人与能识别的标准套餐；不含专属企业部署 |
| QwenWork | 当前用户的国际版桌面登录 | 同上，区域独立 |
| TRAE | 当前用户的国际版桌面登录 | 订阅额度与窗口；不含企业版或会话 Token 统计 |
| TRAE CN | 当前用户的国内版桌面登录 | 同上，区域独立 |
| 千问 | Go 控制中心发起国内官网授权 | Token Plan 与接口可识别的额度；不含本月扣费 |
| QwenCloud | Go 控制中心发起国际官网授权 | 同上，区域独立 |
| 豆包 | 在隔离的浏览器窗口登录豆包官网 | 可识别的订阅额度窗口与额度包 |
| Dola | 在隔离的浏览器窗口登录 Dola 官网 | 可识别的国际订阅／额度包；不是豆包账号 |
| 火山方舟 | 用户提供有只读查询权限的火山云 AK/SK（不是模型 API Key） | 个人 Agent／Coding Plan 与云余额；不含设备登录或专属企业席位 |
| BytePlus ModelArk | 用户提供有只读查询权限的 BytePlus 云 AK/SK | 同上，区域独立 |
| 自定义 JavaScript | 用户显式保存脚本；宿主可提供单个 HTTPS JSON 地址的 GET 结果 | 受限兼容 `fetchUsage(ctx)`，不可访问密钥、任意网络、文件或 shell；并非所有上游脚本可原样运行 |

已转译也只表示源码存在可用的读取路径，不代表与 BrimDeck 的所有兜底、统计与图表行为完全一致，或在每一家真实账户完成验证。新增的中转站类型可以配置多个实例。需要授权的来源仅在插件启用、来源配置完成时采集。手动密钥只存在 Windows 当前用户 DPAPI 文件；同一密钥文件绑定原来源和站点，网络请求不跟随重定向。控制中心的设置管道和状态不传 API 密钥。

Go 控制中心提供插件中的配置卡片，以及可添加至「概览」的展示卡片。只有明确 5h／周的剩余额度参与 20%／10% 默认阈值；余额、积分及用量不会被当成这两档；未知不视为 0。ZCode 有任一时间窗额度（包括 0%）时显示额度；只有两档都缺失才显示剩余 TOKEN，两种数据都未知则显示「—」。多 Agent 可显示独立圆环，自动识别当前支持 Codex、Claude、ZCode、Cursor、DeepSeek Harness；结构化快捷回复仅覆盖部分 Codex 桌面版问答。DeepSeek Harness 显示真实余额，不产生虚构的时间窗额度。中转站余额提醒默认低于 US$1，可在插件中心切换到低于 ¥1；也可选择低于 1 积分；只比较所选单位下明确的余额。达到阈值、其他单位、仅赠送余额、预算、TOKEN 和未知单位均不触发，显示仍保留来源单位。New API 在站点明确配置人民币显示并提供有效计价比例时按站点定义显示，程序不请求外部汇率，也不做跨币种比较。
