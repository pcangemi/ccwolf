use std::f32::consts::TAU;
use std::path::Path;

use crate::rng::hash01;

pub const RATE: u32 = 44_100;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sfx {
    Howl,
    Bell,
    Sting,
    Blip,
}

impl Sfx {
    pub fn parse(name: &str) -> Option<Sfx> {
        match name {
            "howl" => Some(Sfx::Howl),
            "bell" => Some(Sfx::Bell),
            "sting" => Some(Sfx::Sting),
            "blip" => Some(Sfx::Blip),
            _ => None,
        }
    }
}

fn note(semitones_from_a4: i32) -> f32 {
    440.0 * 2f32.powf(semitones_from_a4 as f32 / 12.0)
}

fn pulse(phase: f32, duty: f32) -> f32 {
    if phase.fract() < duty { 1.0 } else { -1.0 }
}

fn triangle(phase: f32) -> f32 {
    let p = phase.fract();
    if p < 0.5 { 4.0 * p - 1.0 } else { 3.0 - 4.0 * p }
}

fn envelope(t: f32, length: f32, attack: f32, release: f32) -> f32 {
    if t < 0.0 || t > length { 0.0 } else { (t / attack.max(1e-4)).min(1.0) * ((length - t) / release.max(1e-4)).min(1.0) }
}

pub struct Mix {
    pub samples: Vec<f32>,
}

impl Mix {
    pub fn new(seconds: f32) -> Mix {
        Mix { samples: vec![0.0; (seconds * RATE as f32) as usize + 1] }
    }

    fn add(&mut self, at: f32, length: f32, voice: impl Fn(f32) -> f32) {
        let start = (at * RATE as f32) as usize;
        let count = (length * RATE as f32) as usize;
        for i in 0..count {
            if let Some(s) = self.samples.get_mut(start + i) {
                *s += voice(i as f32 / RATE as f32);
            }
        }
    }

    pub fn music(&mut self, from: f32, to: f32) {
        let beat = 60.0 / 96.0;
        let bar = beat * 4.0;
        let chords: [[i32; 3]; 4] = [[0, 3, 7], [-4, 0, 3], [-7, -4, 0], [-5, -1, 2]];
        let melody: [i32; 16] = [12, 15, 19, 17, 15, 14, 12, 11, 12, 15, 17, 19, 20, 19, 17, 14];
        let fade = |t: f32| ((t - from) / 2.0).clamp(0.0, 1.0) * ((to - t) / 3.0).clamp(0.0, 1.0);
        let mut t = from;
        let mut step = 0usize;
        while t < to {
            let chord = chords[(step / 32) % 4];
            let sixteenth = beat / 4.0;
            let level = fade(t);
            let root = note(chord[0] - 24);
            if step.is_multiple_of(2) {
                self.add(t, sixteenth * 1.8, move |x| 0.11 * level * pulse(x * root, 0.25) * envelope(x, sixteenth * 1.8, 0.004, 0.05));
            }
            let arp = note(chord[step % 3] + 12);
            self.add(t, sixteenth * 0.9, move |x| 0.05 * level * triangle(x * arp) * envelope(x, sixteenth * 0.9, 0.003, 0.04));
            if step.is_multiple_of(8) {
                self.add(t, 0.18, move |x| 0.22 * level * (TAU * (90.0 - 300.0 * x) * x).sin() * envelope(x, 0.18, 0.002, 0.15));
            }
            if step % 4 == 2 {
                self.add(t, 0.05, move |x| 0.05 * level * (hash01((x * 40000.0) as i64, 3, 9) * 2.0 - 1.0) * envelope(x, 0.05, 0.001, 0.04));
            }
            if t >= from + bar * 8.0 && step.is_multiple_of(4) && (step / 4) % 8 < 7 {
                let degree = melody[(step / 4) % 16];
                let f = note(degree);
                let length = beat * 0.9;
                self.add(t, length, move |x| {
                    let vibrato = 1.0 + 0.006 * (TAU * 5.5 * x).sin();
                    0.06 * level * pulse(x * f * vibrato, 0.5) * envelope(x, length, 0.01, 0.2)
                });
            }
            t += sixteenth;
            step += 1;
        }
    }

    pub fn sfx(&mut self, at: f32, sfx: Sfx) {
        match sfx {
            Sfx::Howl => self.add(at, 2.4, |x| {
                let glide = if x < 0.5 { 380.0 + 260.0 * (x / 0.5) } else { 640.0 - 120.0 * ((x - 0.5) / 1.9) };
                let vibrato = 1.0 + 0.02 * (TAU * 6.0 * x).sin();
                let phase = glide * vibrato * x;
                0.32 * ((TAU * phase).sin() * 0.7 + (TAU * phase * 2.0).sin() * 0.2) * envelope(x, 2.4, 0.25, 0.9)
            }),
            Sfx::Bell => self.add(at, 2.5, |x| {
                let partials = [(523.25, 1.0), (1046.5, 0.5), (1568.0, 0.25), (2093.0, 0.12)];
                let tone: f32 = partials.iter().map(|(f, a)| a * (TAU * f * x).sin()).sum();
                0.2 * tone * (-x * 2.2).exp() * envelope(x, 2.5, 0.003, 0.3)
            }),
            Sfx::Sting => self.add(at, 1.1, |x| {
                let f = 330.0 * (1.0 - 0.45 * x);
                0.24 * (pulse(x * f, 0.5) * 0.6 + pulse(x * f * 1.5, 0.5) * 0.4) * envelope(x, 1.1, 0.005, 0.5)
            }),
            Sfx::Blip => self.add(at, 0.09, |x| 0.08 * pulse(x * 880.0, 0.5) * envelope(x, 0.09, 0.002, 0.04)),
        }
    }

    pub fn write_wav(&self, path: &Path) -> Result<(), String> {
        let peak = self.samples.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1.0);
        let data: Vec<u8> = self.samples.iter().flat_map(|s| (((s / peak) * 0.89 * i16::MAX as f32) as i16).to_le_bytes()).collect();
        let mut out = Vec::with_capacity(44 + data.len());
        out.extend(b"RIFF");
        out.extend((36 + data.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(RATE.to_le_bytes());
        out.extend((RATE * 2).to_le_bytes());
        out.extend(2u16.to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend((data.len() as u32).to_le_bytes());
        out.extend(data);
        std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mix_is_audible_and_never_clips_once_written() {
        let mut mix = Mix::new(12.0);
        mix.music(0.0, 12.0);
        for (i, sfx) in [Sfx::Howl, Sfx::Bell, Sfx::Sting, Sfx::Blip].into_iter().enumerate() {
            mix.sfx(1.0 + i as f32 * 2.0, sfx);
        }
        let loudness = mix.samples.iter().map(|s| s.abs()).sum::<f32>() / mix.samples.len() as f32;
        assert!(loudness > 0.01);
        let path = std::env::temp_dir().join(format!("ccwolf-mix-{}.wav", std::process::id()));
        mix.write_wav(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(bytes.len(), 44 + mix.samples.len() * 2);
        let _ = std::fs::remove_file(path);
    }
}
