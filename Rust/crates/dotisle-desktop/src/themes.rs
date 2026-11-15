pub fn mix(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0., 1.);
    let c = |n: u32| {
        let x = ((a >> n) & 255) as f64;
        let y = ((b >> n) & 255) as f64;
        (x + (y - x) * t).round() as u32
    };
    c(16) << 16 | c(8) << 8 | c(0)
}