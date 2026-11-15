# 点屿字标

开屏、关于页与 README 统一使用用户指定的 Rossela 字形，正文仍为 HarmonyOS Sans。

- `dotisle-wordmark.svg` 与 `dotisle-wordmark-light.svg`：深浅两版，七个字母均为同一份实际字体轮廓，保留字体自身连笔与间距。
- Go 的 `signature_outlines.go` 和 Rust 的 `wordmark_outlines.rs` 使用相同坐标；显示时等比缩放并按可见轮廓居中。
- `wordmark-source.json` 记录作者、字体摘要与来源；仓库不附带 TTF。
- 字体来源为 LetterStuff Typefoundry（2018）的 Rossela-Demo。当前用于本地设计验证；公开分发前须确认作者授予的字标使用权限，转换成矢量不改变授权要求。

应用任务栏和托盘继续使用胶囊图标。
