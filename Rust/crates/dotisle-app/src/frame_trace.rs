//! Opt-in, buffered live-window measurements; no per-frame disk I/O.
use serde_json::json;
use std::{path::PathBuf, time::Instant};

pub struct Trace {
    out: PathBuf,
    scenario: &'static str,
    started: Option<Instant>,
    previous: Option<Instant>,
    samples: Vec<serde_json::Value>,
}
impl Trace {
    pub fn from_args(preview: bool) -> Option<Self> {
        if !preview {
            return None;
        }
        let args = std::env::args().collect::<Vec<_>>();
        let (index, scenario) =
            args.iter()
                .enumerate()
                .find_map(|(i, arg)| match arg.as_str() {
                    "--perf-run" => Some((i, "deck")),
                    "--perf-startup" => Some((i, "startup")),
                    "--perf-interactions" => Some((i, "interactions")),
                    _ => None,
                })?;
        let out = args.get(index + 1).map(PathBuf::from)?;
        Some(Self {
            out,
            scenario,
            started: None,
            previous: None,
            samples: Vec::with_capacity(1200),
        })
    }
    pub fn elapsed(&self) -> f64 {
        self.started.map_or(0., |at| at.elapsed().as_secs_f64())
    }
    pub fn is_startup(&self) -> bool {
        self.scenario == "startup"
    }
    pub fn is_interactive(&self) -> bool {
        self.scenario == "interactions"
    }
    pub fn duration(&self) -> f64 {
        if self.is_startup() {
            2.
        } else {
            10.
        }
    }
    pub fn target(&self) -> f64 {
        // Repeatable wheel-like targets, including a reversal, through all five cards.
        let t = self.elapsed();
        let step = (t / 0.32).floor();
        let cards = if step < 17. { step } else { 34. - step };
        cards * crate::overview::STEP
    }
    pub fn record(
        &mut self,
        at: Instant,
        build_ms: f64,
        submit_ms: f64,
        scroll: f64,
        size: (u32, u32),
        scale: f64,
        ok: bool,
        stages: [f64; 7],
        signature_age: Option<f64>,
    ) {
        let start = *self.started.get_or_insert(at);
        let elapsed = at.duration_since(start).as_secs_f64();
        let interval = self
            .previous
            .replace(at)
            .map(|last| at.duration_since(last).as_secs_f64() * 1000.);
        let measuring = if self.is_startup() {
            signature_age.is_some()
        } else {
            elapsed >= 2.
        };
        if measuring {
            self.samples.push(json!({
                "elapsed_s": elapsed,
                "interval_ms": interval,
                "build_ms": build_ms,
                "submit_present_cpu_ms": submit_ms,
                "scroll": scroll,
                "physical_size": size,
                "scale": scale,
                "ok": ok,
                "signature_age": signature_age,
                "acquire_ms": stages[0],
                "encode_ms": stages[1],
                "queue_ms": stages[2],
                "present_ms": stages[3],
                "prepare_ms": stages[4],
                "replay_ms": stages[5],
                "hybrid_ms": stages[6],
            }));
        }
    }
    pub fn finish(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.out).map_err(|e| e.to_string())?;
        let stats = |key: &str| {
            let mut values = self
                .samples
                .iter()
                .filter_map(|v| v[key].as_f64())
                .collect::<Vec<_>>();
            values.sort_by(f64::total_cmp);
            let q = |p: f64| {
                values
                    .get(((values.len().saturating_sub(1)) as f64 * p).round() as usize)
                    .copied()
                    .unwrap_or(0.)
            };
            json!({"p50":q(0.50),"p95":q(0.95),"p99":q(0.99)})
        };
        let report = json!({"success":self.samples.len()>30 && self.samples.iter().all(|s|s["ok"]==true),
            "scenario":self.scenario,
            "scope":"live DX12 swapchain; startup samples the visible signature; other runs exclude 2s warmup; CPU submit/present is not GPU execution time",
            "frames":self.samples.len(),"interval_ms":stats("interval_ms"),"build_ms":stats("build_ms"),
            "submit_present_cpu_ms":stats("submit_present_cpu_ms"),"samples":self.samples});
        std::fs::write(
            self.out.join("frames.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .map_err(|e| e.to_string())
    }
}
