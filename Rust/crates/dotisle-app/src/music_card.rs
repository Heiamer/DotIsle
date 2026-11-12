//! A fixed two-column music grid and an interruptible, bounded lyric viewport.
use crate::{
    draw::{self, rect},
    model::Model,
    motion::Spring,
    views::{self, ViewState},
    widgets::Ui,
};
use vello::{
    kurbo::{Affine, Rect},
    peniko::{BlendMode, Compose, Fill, Gradient, Mix},
};

pub struct Grid {
    pub summary: Rect,
    pub cover: Rect,
    pub title: Rect,
    pub artist: Rect,
    pub lyrics: Rect,
    pub progress: Rect,
    pub controls: Rect,
}

impl Grid {
    pub fn new(width: f64, height: f64) -> Self {
        let inset = 24.;
        let column = ((width * 0.24 / 8.).round() * 8.).clamp(192., 224.);
        let right = inset + column + 32.;
        let cover_size = (height - 222.).clamp(64., 112.);
        let title_y = 64. + cover_size + 12.;
        let controls = rect(inset + (column - 164.) / 2., height - 78., 164., 52.);
        Self {
            summary: rect(inset, 64., column, height - 88.),
            cover: rect(
                inset + (column - cover_size) / 2.,
                64.,
                cover_size,
                cover_size,
            ),
            title: rect(inset, title_y, column, 28.),
            artist: rect(inset, title_y + 34., column, 18.),
            lyrics: rect(right, 64., width - inset - right, height - 156.),
            progress: rect(right, height - 64., width - inset - right, 24.),
            controls,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PresentedLine {
    pub text: String,
    pub translation: String,
    pub offset: f64,
    pub emphasis: f64,
    pub active: bool,
}

struct MovingLine {
    index: i64,
    text: String,
    translation: String,
    offset: Spring,
    emphasis: Spring,
}

#[derive(Default)]
pub struct LyricsMotion {
    track: String,
    index: i64,
    current: String,
    lines: Vec<MovingLine>,
}

impl LyricsMotion {
    pub fn tick(&mut self, model: &Model, dt: f64, visible: bool) -> bool {
        let frame = &model.snapshot["frame"];
        let track = &model.snapshot["track"];
        let identity = format!("{}\0{}\0{}", track["id"], track["title"], track["artist"]);
        let current = model.current_lyric();
        let index = frame["currentLineIndex"].as_i64().unwrap_or_else(|| {
            if self.current == current {
                self.index
            } else {
                self.index + 1
            }
        });
        let reset = self.track != identity
            || !visible
            || model.prefs.reduced_motion
            || (index == self.index && self.current != current);
        let previous = frame["previousLine"].as_str().unwrap_or("");
        let next = frame["nextLine"].as_str().unwrap_or("");
        let translation = if model.prefs.translation {
            frame["currentTranslation"].as_str().unwrap_or("")
        } else {
            ""
        };
        if reset {
            self.lines.clear();
        }
        self.track = identity;
        let changed_line = self.current != current || self.index != index;
        let direction = if index < self.index { -1. } else { 1. };
        self.current = current.into();
        self.index = index;
        let mut moving = false;
        for (slot, text, secondary) in
            [(-1, previous, ""), (0, current, translation), (1, next, "")]
        {
            if text.trim().is_empty()
                || (slot != 0 && (!model.track() || frame["isPureMusic"] == true))
            {
                continue;
            }
            let row_index = index + slot;
            if let Some(line) = self
                .lines
                .iter_mut()
                .find(|line| line.index == row_index && line.text == text)
            {
                line.translation = secondary.into();
            } else {
                let initial = if reset || !changed_line {
                    slot as f64
                } else {
                    slot as f64 + direction
                };
                self.lines.push(MovingLine {
                    index: row_index,
                    text: text.into(),
                    translation: secondary.into(),
                    offset: Spring::new(initial),
                    emphasis: Spring::new(if slot == 0 && reset { 1. } else { 0. }),
                });
            }
        }
        for line in &mut self.lines {
            line.offset.target = (line.index - index).clamp(-3, 3) as f64;
            line.emphasis.target = if line.index == index && line.text == current {
                1.
            } else {
                0.
            };
            moving |= line.offset.tick(dt, reset);
            moving |= line.emphasis.tick(dt, reset);
        }
        self.lines.retain(|line| {
            let relative = line.index - index;
            let wanted = match relative {
                -1 => line.text == previous && !previous.trim().is_empty(),
                0 => line.text == current,
                1 => line.text == next && !next.trim().is_empty(),
                _ => false,
            };
            wanted || (relative.abs() == 2 && !line.offset.settled())
        });
        moving
    }

    pub fn presentation(&self) -> Vec<PresentedLine> {
        self.lines
            .iter()
            .map(|line| PresentedLine {
                text: line.text.clone(),
                translation: line.translation.clone(),
                offset: line.offset.value,
                emphasis: line.emphasis.value.clamp(0., 1.),
                active: line.index == self.index && line.text == self.current,
            })
            .collect()
    }
}

fn static_lines(model: &Model) -> Vec<PresentedLine> {
    let mut motion = LyricsMotion::default();
    motion.tick(model, 0., false);
    motion.presentation()
}

/// Wrap the active lyric into at most two rows. The second row has a bounded
/// ellipsis if an unbreakable token or a long translation exceeds the viewport.
fn wrap(ui: &mut Ui, text: &str, width: f64, size: f32) -> Vec<String> {
    let mut rows = vec![];
    let mut row = String::new();
    for c in text.chars() {
        let mut candidate = row.clone();
        candidate.push(c);
        if !row.is_empty() && ui.measure_alpha(&candidate, size, true) > width && rows.is_empty() {
            rows.push(row.trim_end().to_owned());
            row.clear();
        }
        row.push(c);
    }
    rows.push(row.trim().to_owned());
    rows
}

fn lyric_view(ui: &mut Ui, model: &Model, area: Rect, state: &ViewState) {
    let lines = state
        .lyric_lines
        .clone()
        .filter(|lines| !lines.is_empty())
        .unwrap_or_else(|| static_lines(model));
    let size = 25.;
    let active = lines.iter().find(|line| line.active);
    let rows = active
        .map(|line| wrap(ui, &line.text, area.width(), size))
        .unwrap_or_default();
    let translated = active.is_some_and(|line| !line.translation.trim().is_empty());
    let active_height = rows.len().max(1) as f64 * 32. + if translated { 23. } else { 0. };
    let anchor = area.center().y - active_height / 2.;
    let ink = ui.s.on_surface;
    // The clip and opacity mask belong to one isolated surface. Per-line blur
    // must not leak into the cover, controls, or another deck card.
    ui.scene.push_isolated_clip_layer(Affine::IDENTITY, &area);
    for line in lines {
        let scale = 0.88 + 0.12 * line.emphasis;
        let spacing = if line.offset < 0. {
            42.
        } else {
            active_height + 14.
        };
        let y = anchor + line.offset * spacing;
        let opacity = (0.27 + 0.73 * line.emphasis) * (2. - line.offset.abs()).clamp(0., 1.);
        if opacity < 0.005 || y > area.y1 + 8. || y + 90. < area.y0 {
            continue;
        }
        let transform = Affine::translate((area.x0, y)) * Affine::scale(scale);
        let old = std::mem::take(ui.scene);
        let main_rows = if line.active {
            wrap(ui, &line.text, area.width() / scale, size)
        } else {
            vec![line.text.clone()]
        };
        let count = line.text.chars().count().max(1) as f64;
        let scan = if line.active && model.prefs.word_highlight {
            model.snapshot["frame"]["wordScanProgress"]
                .as_f64()
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(0., 1.))
        } else {
            None
        };
        let mut consumed = 0.;
        for (i, text) in main_rows.iter().enumerate() {
            let row_y = i as f64 * 32.;
            ui.label_alpha(
                text,
                0.,
                row_y,
                size,
                ink,
                opacity * if scan.is_some() { 0.4 } else { 1. },
                true,
                area.width() / scale,
            );
            if let Some(scan) = scan {
                let chars = text.chars().collect::<Vec<_>>();
                let lit = (scan * count - consumed).clamp(0., chars.len() as f64);
                let whole = lit.floor() as usize;
                let prefix = chars[..whole].iter().collect::<String>();
                let mut width = ui.measure_alpha(&prefix, size, true);
                if let Some(c) = chars.get(whole) {
                    width += ui.measure_alpha(&c.to_string(), size, true) * lit.fract();
                }
                if width > 0. {
                    ui.scene.push_clip_layer(
                        Affine::IDENTITY,
                        &rect(0., row_y - 3., width.min(area.width() / scale), 38.),
                    );
                    ui.label_alpha(
                        text,
                        0.,
                        row_y,
                        size,
                        ink,
                        opacity,
                        true,
                        area.width() / scale,
                    );
                    ui.scene.pop_layer();
                }
                consumed += chars.len() as f64;
            }
        }
        if line.active && !line.translation.trim().is_empty() {
            ui.label_alpha(
                &line.translation,
                0.,
                main_rows.len() as f64 * 32. + 3.,
                13.,
                ink,
                opacity * 0.65,
                false,
                area.width() / scale,
            );
        }
        let row_scene = std::mem::replace(ui.scene, old);
        let blur = if model.prefs.reduced_motion {
            0.
        } else {
            (1. - line.emphasis) * 1.65
        };
        if blur > 0.05 {
            ui.scene
                .push_gaussian_layer(Affine::IDENTITY, &area.inflate(8., 8.), blur);
        }
        ui.scene.append(&row_scene, Some(transform));
        if blur > 0.05 {
            ui.scene.pop_layer();
        }
    }
    let mask = Gradient::new_linear((area.x0, area.y0), (area.x0, area.y1)).with_stops([
        (0., draw::alpha(0xffffff, 0.)),
        (0.14, draw::alpha(0xffffff, 1.)),
        (0.84, draw::alpha(0xffffff, 1.)),
        (1., draw::alpha(0xffffff, 0.)),
    ]);
    ui.scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::DestIn),
        1.,
        Affine::IDENTITY,
        &area,
    );
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &mask, None, &area);
    ui.scene.pop_layer();
    ui.scene.pop_layer();
}

pub fn render(ui: &mut Ui, model: &Model, width: f64, height: f64, state: &ViewState) {
    let grid = Grid::new(width, height);
    let ink = ui.s.on_surface;
    views::record(ui, model, grid.cover, false);
    ui.center(model.title(), grid.title, 20., ink, true);
    ui.center(
        model.artist(),
        grid.artist,
        12.,
        ui.s.on_surface_variant,
        false,
    );
    lyric_view(ui, model, grid.lyrics, state);
    views::player_controls(ui, model, grid.controls.x0, grid.controls.y0, false);
    views::progress(ui, model, grid.progress, state);
}

pub fn verify(data: &std::path::Path) -> Result<(), String> {
    for width in [756., 828., 980.] {
        for height in [280., 310., 430.] {
            let grid = Grid::new(width, height);
            if grid.lyrics.x0 != grid.progress.x0
                || grid.lyrics.x1 != grid.progress.x1
                || grid.artist.y1 + 8. > grid.controls.y0
                || grid.lyrics.y1 + 16. > grid.progress.y0
                || (grid.controls.center().y - grid.progress.center().y).abs() > 0.01
                || grid.summary.x1 + 32. != grid.lyrics.x0
            {
                return Err("music card broke its column guides or vertical clearances".into());
            }
        }
    }
    let mut model = Model::new(data.to_path_buf(), true);
    model.snapshot["frame"] = serde_json::json!({"previousLine":"A","currentLine":"B","nextLine":"C","currentLineIndex":1});
    let mut motion = LyricsMotion::default();
    motion.tick(&model, 0., false);
    model.snapshot["frame"] = serde_json::json!({"previousLine":"B","currentLine":"C","nextLine":"D","currentLineIndex":2});
    motion.tick(&model, 1. / 60., true);
    let before = motion.presentation();
    let moving = before
        .iter()
        .find(|row| row.text == "B")
        .ok_or("outgoing lyric missing")?;
    if !(moving.offset < 0. && moving.offset > -1.) {
        return Err("line change snapped instead of scrolling".into());
    }
    model.snapshot["frame"] = serde_json::json!({"previousLine":"A","currentLine":"B","nextLine":"C","currentLineIndex":1});
    motion.tick(&model, 1. / 60., true);
    for _ in 0..180 {
        motion.tick(&model, 1. / 60., true);
    }
    if motion.tick(&model, 1. / 60., true) || motion.lines.len() > 3 {
        return Err("settled lyrics kept animating or retaining old lines".into());
    }
    model.prefs.reduced_motion = true;
    model.snapshot["frame"]["currentLine"] = serde_json::json!("D");
    motion.tick(&model, 1. / 60., true);
    if motion
        .presentation()
        .iter()
        .any(|line| line.active && line.offset != 0.)
    {
        return Err("reduced motion retained lyric travel".into());
    }
    Ok(())
}

/// Capture bounded long-text fallbacks and real spring presentation frames.
pub fn capture(
    out: &std::path::Path,
    text: &mut crate::draw::Text,
    gpu: &mut crate::gpu::Gpu,
) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut model = Model::new(out.join("capture-data"), true);
    model.tutorial_checked = true;
    model.tutorial = None;
    model.toast = None;
    model.prefs.control_center_theme = "Original".into();
    crate::themes::end_transition();
    for dark in [false, true] {
        model.prefs.dark = dark;
        for (width, height) in [(900., 680.), (1200., 800.)] {
            for (name, frame) in [
                (
                    "long",
                    serde_json::json!({"previousLine":"上一句，留在此刻的上方","currentLine":"即使是一句很长很长的歌词，也要在自己的参考线里保持舒展，留出呼吸的位置","currentTranslation":"A long translation stays inside the lyric column and above the playback controls.","nextLine":"下一句，慢慢走进视线","currentLineIndex":1,"wordScanProgress":0.63}),
                ),
                (
                    "empty",
                    serde_json::json!({"currentLine":"","currentLineIndex":0}),
                ),
                (
                    "instrumental",
                    serde_json::json!({"currentLine":"","isPureMusic":true,"currentLineIndex":0}),
                ),
            ] {
                model.snapshot["frame"] = frame;
                let view = views::render(&model, text, width, height, &ViewState::default());
                gpu.capture(
                    &view.scene,
                    width as u32,
                    height as u32,
                    &out.join(format!("{name}-{width:.0}-{dark}.png")),
                )?;
            }
        }
    }
    model.prefs.dark = false;
    let mut motion = LyricsMotion::default();
    for frame in 0..64 {
        let (previous, current, next, index) = if frame < 12 {
            (
                "把远方写成一封信",
                "海风吹过，晚霞落在你身旁",
                "把今天的温柔，轻轻收藏",
                1,
            )
        } else if frame < 28 {
            (
                "海风吹过，晚霞落在你身旁",
                "把今天的温柔，轻轻收藏",
                "下一句，慢慢走进视线",
                2,
            )
        } else {
            (
                "把远方写成一封信",
                "海风吹过，晚霞落在你身旁",
                "把今天的温柔，轻轻收藏",
                1,
            )
        };
        model.snapshot["frame"] = serde_json::json!({"previousLine":previous,"currentLine":current,"nextLine":next,"currentLineIndex":index,"wordScanProgress":(frame%28)as f64/28.});
        motion.tick(&model, 1. / 60., frame != 0);
        if frame % 2 == 0 {
            let view = views::render(
                &model,
                text,
                900.,
                680.,
                &ViewState {
                    lyric_lines: Some(motion.presentation()),
                    ..Default::default()
                },
            );
            gpu.capture(
                &view.scene,
                900,
                680,
                &out.join(format!("motion-{frame:03}.png")),
            )?;
        }
    }
    Ok(())
}
