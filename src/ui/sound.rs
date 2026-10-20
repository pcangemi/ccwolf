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

const MELODY: [&[(i32, usize)]; 8] = [
    &[(7, 2), (12, 2), (15, 1), (14, 1), (12, 2)],
    &[(15, 3), (12, 1), (8, 2), (12, 2)],
    &[(10, 2), (7, 1), (10, 1), (15, 2), (14, 1), (15, 1)],
    &[(17, 3), (14, 1), (10, 4)],
    &[(7, 2), (12, 2), (15, 1), (14, 1), (12, 1), (15, 1)],
    &[(17, 2), (15, 2), (12, 2), (8, 2)],
    &[(10, 1), (12, 1), (15, 2), (19, 2), (15, 2)],
    &[(14, 3), (12, 1), (10, 2), (14, 2)],
];

fn melody_note(bar: usize, eighth: usize) -> Option<(i32, usize)> {
    let mut at = 0;
    for &(tone, length) in MELODY[bar % MELODY.len()] {
        if at == eighth {
            return Some((tone, length));
        }
        at += length;
    }
    None
}

const MUSIC_GAIN: f32 = 0.8;
const DUCKED: f32 = 0.35;

fn duck(t: f32, ducks: &[(f32, f32)]) -> f32 {
    ducks.iter().fold(1.0f32, |gain, &(at, length)| {
        let into = ((t - (at - 0.25)) / 0.25).clamp(0.0, 1.0);
        let out = ((at + length + 0.6 - t) / 0.6).clamp(0.0, 1.0);
        gain.min(1.0 - (1.0 - DUCKED) * into.min(out))
    })
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

    pub fn music(&mut self, from: f32, to: f32, ducks: &[(f32, f32)]) {
        let mut bus = Mix { samples: vec![0.0; self.samples.len()] };
        let eighth = 60.0 / 120.0 / 2.0;
        let bass_roots = [-24, -28, -21, -26];
        let stabs: [[i32; 2]; 4] = [[-9, -5], [-12, -9], [-5, -2], [-10, -7]];
        let arps: [[i32; 3]; 4] = [[0, 3, 7], [-4, 0, 3], [3, 7, 10], [-2, 2, 5]];
        let mut t = from;
        let mut step = 0usize;
        while t < to {
            let bar = step / 8;
            let chord = (bar + 2) % 4;
            let in_bar = step % 8;
            let bass = note(bass_roots[chord] + if in_bar % 2 == 1 { 12 } else { 0 });
            bus.add(t, eighth * 0.8, move |x| 0.17 * triangle(x * bass) * envelope(x, eighth * 0.8, 0.004, 0.04));
            if in_bar.is_multiple_of(4) {
                bus.add(t, 0.16, |x| 0.2 * (TAU * (120.0 - 220.0 * x) * x).sin() * envelope(x, 0.16, 0.002, 0.13));
            }
            if in_bar % 4 == 2 {
                bus.add(t, 0.13, |x| {
                    let noise = hash01((x * 30000.0) as i64, 5, 11) * 2.0 - 1.0;
                    (0.06 * noise + 0.05 * triangle(x * 190.0)) * (-x * 28.0).exp() * envelope(x, 0.13, 0.001, 0.05)
                });
            }
            if in_bar % 2 == 1 {
                bus.add(t, 0.03, |x| 0.018 * (hash01((x * 44000.0) as i64, 7, 13) * 2.0 - 1.0) * envelope(x, 0.03, 0.001, 0.025));
                for tone in stabs[chord] {
                    let f = note(tone);
                    bus.add(t, 0.11, move |x| 0.022 * pulse(x * f, 0.5) * (-x * 18.0).exp() * envelope(x, 0.11, 0.003, 0.04));
                }
            }
            let section = bar.checked_sub(2).map(|b| b % 12);
            match section {
                Some(b) if b < 8 => {
                    if let Some((tone, eighths)) = melody_note(b, in_bar) {
                        let f = note(tone);
                        let length = eighth * eighths as f32 * 0.92;
                        bus.add(t, length, move |x| {
                            let vibrato = 1.0 + 0.005 * (TAU * 5.5 * x).sin() * ((x - 0.15) / 0.2).clamp(0.0, 1.0);
                            0.05 * pulse(x * f * vibrato, 0.25) * envelope(x, length, 0.008, 0.06)
                        });
                    }
                }
                Some(_) => {
                    for sixteenth in 0..2 {
                        let f = note(arps[chord][(in_bar * 2 + sixteenth) % 3] + 12);
                        let at = t + sixteenth as f32 * eighth / 2.0;
                        bus.add(at, eighth * 0.45, move |x| 0.035 * triangle(x * f) * envelope(x, eighth * 0.45, 0.003, 0.03));
                    }
                }
                None => {}
            }
            t += eighth;
            step += 1;
        }
        bus.soften(4200.0);
        bus.soften(4200.0);
        for (i, sample) in bus.samples.iter().enumerate() {
            let t = i as f32 / RATE as f32;
            let fade = ((t - from) / 2.0).clamp(0.0, 1.0) * ((to - t) / 3.0).clamp(0.0, 1.0);
            self.samples[i] += sample * MUSIC_GAIN * fade * duck(t, ducks);
        }
    }

    fn soften(&mut self, cutoff: f32) {
        let a = 1.0 - (-TAU * cutoff / RATE as f32).exp();
        let mut y = 0.0;
        for s in &mut self.samples {
            y += a * (*s - y);
            *s = y;
        }
    }

    pub fn sample(&mut self, at: f32, sound: &[f32], peak: f32) {
        let loudest = sound.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        if loudest == 0.0 {
            return;
        }
        let start = (at * RATE as f32) as usize;
        for (i, s) in sound.iter().enumerate() {
            if let Some(out) = self.samples.get_mut(start + i) {
                *out += s / loudest * peak;
            }
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
                0.2 * (triangle(x * f) * 0.6 + triangle(x * f * 1.5) * 0.3 + pulse(x * f, 0.5) * 0.1) * envelope(x, 1.1, 0.005, 0.5)
            }),
            Sfx::Blip => self.add(at, 0.09, |x| 0.07 * triangle(x * 880.0) * envelope(x, 0.09, 0.002, 0.04)),
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
        mix.music(0.0, 12.0, &[]);
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

    fn loudness(mix: &Mix, from: f32, to: f32) -> f32 {
        let window = &mix.samples[(from * RATE as f32) as usize..(to * RATE as f32) as usize];
        (window.iter().map(|s| s * s).sum::<f32>() / window.len() as f32).sqrt()
    }

    #[test]
    fn the_music_steps_back_while_the_wolf_howls() {
        let mut plain = Mix::new(20.0);
        plain.music(0.0, 20.0, &[]);
        let mut ducked = Mix::new(20.0);
        ducked.music(0.0, 20.0, &[(8.0, 4.0)]);
        assert!(loudness(&ducked, 8.5, 11.5) < loudness(&plain, 8.5, 11.5) * 0.5);
        assert!((loudness(&ducked, 14.0, 16.0) - loudness(&plain, 14.0, 16.0)).abs() < 1e-4);
    }

    #[test]
    fn the_melody_fills_every_bar_exactly() {
        for bar in MELODY {
            assert_eq!(bar.iter().map(|(_, length)| length).sum::<usize>(), 8);
        }
    }

    #[test]
    fn a_recorded_sound_lands_at_its_cue_with_the_asked_peak() {
        let mut mix = Mix::new(2.0);
        mix.sample(1.0, &[0.0, 0.5, -2.0, 1.0], 0.4);
        let at = RATE as usize;
        assert_eq!(&mix.samples[at..at + 4], &[0.0, 0.1, -0.4, 0.2]);
        assert!(mix.samples[..at].iter().all(|s| *s == 0.0));
    }
}
