# 点屿字标

对外展示统一使用这版单色 Dotisle 手写字标，与设置开场和关于页共用相同的矢量轮廓。

- `dotisle-wordmark.svg`：深色笔画，适用于浅色背景。
- `dotisle-wordmark-light.svg`：浅色笔画，适用于深色背景。
- README 使用 `<picture>` 按显示主题切换；按比例缩放并保留四周留白。
- 形状来源：`Rust/crates/dotisle-render/src/brand_signature.rs` 的 `OUTLINES`，应用开场使用同稿沿笔路书写。

此字标用于 README、文档与宣传展示。应用任务栏、托盘等小尺寸方形位置继续采用胶囊图标。
