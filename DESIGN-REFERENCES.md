# 点屿设计参考 · 2026-10-29

- mpvRx 3.0.0：M3E 的组件层级、连接形状和轻量玻璃播放控件。https://github.com/Riteshp2001/mpvRx/releases/tag/v3.0.0
- Zafiro：公开采用 Material 3 Expressive 与 Apple Liquid Glass，关注悬浮入口与面板。https://github.com/niki914/zafiro#modern-ui-design-language
- Backdrop / AndroidLiquidGlass：玻璃边缘与交互组件示例。https://github.com/Kyant0/AndroidLiquidGlass

本轮只借鉴交互与视觉原则；求解器、Windows 采样与折射合成由本项目实现，没有复制这些项目的控件或代码。


## 3.2.3：MIUIX 与 Swiss

- MIUIX：https://github.com/compose-miuix-ui/miuix 。仅参考其设计与交互，在现有 Rust/Vello 中实现；不引入 Kotlin 运行时或第三方执行代码。
- 对照官方 Colors.kt、Card.kt、Switch.kt 与 Button.kt：浅色强调 #3482FF、#F7F7F7 页面、白色分组、16 dp 连续圆角、49×28 dp 开关与 20 dp 白色拇指。保留原生设置的命中区域和操作协议。
- Swiss：左对齐无衬线、编号、非对称网格、黑白纸色与红色强调，音乐版式重新排列；几何图标在本项目中绘制。优先系统 Arial，中文使用已有字体回退。
- 所有主题均使用原始专辑封面，不再做风格化图片转换。原图只按显示尺寸常规缩放和裁切。
