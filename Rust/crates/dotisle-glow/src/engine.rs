// Optional halo states. All three effects default off; Mini never paints a halo.
// Audio response reads the endpoint peak meter, not recorded audio or microphone input.
use lyric_core::shared_motion::Spring;
use std::time::{Duration, Instant};

use windows::Win32::{
    Media::Audio::{
        eMultimedia, eRender, Endpoints::IAudioMeterInformation, IMMDeviceEnumerator,
        MMDeviceEnumerator,
    },
    System::Com::{CoCreateInstance, CLSCTX_ALL},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    #[default]
    Off,
    MusicBreath,
    MusicRhythm,
    Timer,
    Update,
}
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Frame {
    pub kind: Kind,
    pub strength: f64,
    pub phase: f64,
}
pub struct State {
    kind: Kind,
    visible: Spring,
    envelope: Spring,
    meter: Option<IAudioMeterInformation>,
    last_sample: Instant,
    last_bind: Instant,
    peak: f64,
}
impl Default for State {
    fn default() -> Self {
        Self {
            kind: Kind::Off,
            visible: Spring::new(0.),
            envelope: Spring::new(0.),
            meter: None,
            last_sample: Instant::now(),
            last_bind: Instant::now() - Duration::from_secs(3),
            peak: 0.,
        }
    }
}
impl State {
    fn sample_peak(&mut self) -> f64 {
        if self.last_sample.elapsed() < Duration::from_millis(25) {
            return self.peak;
        }
        self.last_sample = Instant::now();
        if self.meter.is_none() && self.last_bind.elapsed() >= Duration::from_secs(2) {
            self.last_bind = Instant::now();
            self.meter = unsafe {
                (CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL)
                    .ok())
                .and_then(|enumerator| {
                    enumerator
                        .GetDefaultAudioEndpoint(eRender, eMultimedia)
                        .ok()
                })
                .and_then(|endpoint| {
                    endpoint
                        .Activate::<IAudioMeterInformation>(CLSCTX_ALL, None)
                        .ok()
                })
            };
        }
        self.peak = if let Some(meter) = &self.meter {
            match unsafe { meter.GetPeakValue() } {
                Ok(value) if value.is_finite() => value.clamp(0., 1.) as f64,
                _ => {
                    self.meter = None;
                    0.
                }
            }
        } else {
            0.
        };
        self.peak
    }
    pub fn tick(&mut self, dt: f64, target: Kind, animate: bool) -> bool {
        if target != Kind::Off {
            self.kind = target;
        }
        self.visible.target = if target == Kind::Off { 0. } else { 1. };
        self.envelope.target = if target == Kind::MusicRhythm {
            self.sample_peak().sqrt()
        } else {
            0.
        };
        if target != Kind::MusicRhythm {
            self.meter = None;
            self.peak = 0.;
        }
        let changed = self.visible.tick(dt, !animate) | self.envelope.tick(dt, !animate);
        if self.visible.value < 0.002 && target == Kind::Off {
            self.kind = Kind::Off;
        }
        changed || (animate && self.kind != Kind::Off)
    }
    pub fn active(&self) -> bool {
        self.kind != Kind::Off || self.visible.value > 0.002
    }
    pub fn frame(&self, time_ms: f64, animate: bool) -> Frame {
        let breath = if animate {
            0.65 + 0.2 * (time_ms / 2400. * std::f64::consts::TAU).sin()
        } else {
            0.6
        };
        let amount = match self.kind {
            Kind::MusicRhythm => 0.12 + 0.88 * self.envelope.value.clamp(0., 1.),
            Kind::MusicBreath | Kind::Timer => breath,
            Kind::Update => 0.85,
            Kind::Off => 0.,
        };
        Frame {
            kind: self.kind,
            strength: (self.visible.value * amount).clamp(0., 1.),
            phase: if animate {
                time_ms / 3800. * std::f64::consts::TAU
            } else {
                0.
            },
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn off_does_not_bind_audio_and_shutdown_settles() {
        let mut state = State::default();
        assert!(!state.tick(1. / 120., Kind::Off, true));
        assert!(state.meter.is_none());
        for _ in 0..120 {
            state.tick(1. / 120., Kind::Timer, true);
        }
        assert!(state.frame(0., true).strength > 0.5);
        for _ in 0..240 {
            state.tick(1. / 120., Kind::Off, true);
        }
        assert!(!state.active());
        assert!(state.meter.is_none());
    }

}
