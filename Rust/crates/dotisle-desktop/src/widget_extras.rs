use crate::widgets::Ui;
pub(crate) fn text_lines(
    ui: &mut Ui,
    value: &str,
    x: f64,
    y: f64,
    width: f64,
    lines: usize,
    size: f32,
    color: u32,
) {
    let mut line = String::new();
    let mut row = 0;
    for ch in value.chars().chain(std::iter::once('\n')) {
        let mut next = line.clone();
        next.push(ch);
        if ch == '\n' || ui.measure_alpha(&next, size, false) > width {
            ui.label(
                &line,
                x,
                y + row as f64 * (size as f64 * 1.5),
                size,
                color,
                false,
                width,
            );
            row += 1;
            if row >= lines {
                return;
            }
            line.clear();
        }
        if ch != '\n' {
            line.push(ch);
        }
    }
}