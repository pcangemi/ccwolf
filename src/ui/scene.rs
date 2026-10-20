use std::f32::consts::PI;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::game::Role;
use crate::model::Model;
use crate::rng::{hash01, noise};
use crate::ui::canvas::{Canvas, Rgb, big_text, label, ramp, text_width};

const HUMAN: [&str; 12] = ["..hhh..", ".hhhhh.", ".hssss.", ".seses.", ".sssss.", "..sss..", ".ccccc.", "ccccccc", "ccccccc", ".ccccc.", ".cc.cc.", ".bb.bb."];
const WOLF: [&str; 12] = ["f.....f", "ff...ff", "fffffff", "fyfffyf", "fffffff", ".fnnnf.", ".ccccc.", "ccccccc", "ccccccc", ".ccccc.", ".ff.ff.", ".ff.ff."];
const TOMB: [&str; 8] = ["..ggg..", ".ggggg.", ".gg+gg.", ".g+++g.", ".gg+gg.", ".ggggg.", ".ggggg.", "ddddddd"];

const HAIR: [u32; 6] = [0x3b2a20, 0x1d1a1f, 0xc9a25c, 0x8a8f99, 0x7a3b1e, 0x4e3527];
const SKIN: [u32; 5] = [0xf1c9a5, 0xd9a57e, 0xb07a52, 0x8a5a3c, 0xf5d7c0];

pub struct Seat {
    pub name: String,
    pub model: Model,
    pub role: Role,
    pub alive: bool,
    pub dissolve: Option<f32>,
}

pub struct Scene<'a> {
    pub seats: &'a [Seat],
    pub night: f32,
    pub day: u32,
    pub t: f32,
    pub speaker: Option<usize>,
    pub thinking: Option<usize>,
    pub mark: Option<(usize, Rgb)>,
    pub unmasked: Vec<bool>,
    pub banner: Option<(String, f32)>,
    pub flash: Option<(Rgb, f32)>,
}

fn sky(c: &mut Canvas, horizon: i32, night: f32, t: f32) {
    let day_top = Rgb::hex(0x5b9be0);
    let day_low = Rgb::hex(0xcfe6f5);
    let dusk_top = Rgb::hex(0x3a3478);
    let dusk_low = Rgb::hex(0xf0905a);
    let night_top = Rgb::hex(0x04050f);
    let night_low = Rgb::hex(0x1b2246);
    let top = ramp(&[(0.0, day_top), (0.5, dusk_top), (1.0, night_top)], night);
    let low = ramp(&[(0.0, day_low), (0.5, dusk_low), (1.0, night_low)], night);
    for y in 0..horizon.max(1) {
        let row = top.mix(low, (y as f32 / horizon.max(1) as f32).powf(1.4));
        for x in 0..c.w {
            c.set(x, y, row);
        }
    }
    let stars = ((night - 0.45) * 1.8).clamp(0.0, 1.0);
    if stars > 0.0 {
        for y in 0..horizon {
            for x in 0..c.w {
                let h = hash01(x as i64, y as i64, 7);
                if h < 0.012 {
                    let twinkle = 0.55 + 0.45 * (t * 1.7 + h * 900.0).sin();
                    c.glow(x, y, Rgb(200.0, 215.0, 255.0), stars * twinkle * (0.5 + h * 40.0).min(1.0));
                }
            }
        }
        let cycle = t % 11.0;
        if cycle < 0.9 {
            let p = cycle / 0.9;
            let (sx, sy) = (c.w as f32 * 0.15, horizon as f32 * 0.1);
            for k in 0..9 {
                let q = (p - k as f32 * 0.03).max(0.0);
                c.glow(
                    (sx + q * c.w as f32 * 0.35) as i32,
                    (sy + q * horizon as f32 * 0.35) as i32,
                    Rgb(255.0, 255.0, 255.0),
                    stars * (1.0 - k as f32 / 9.0) * (1.0 - p),
                );
            }
        }
    }
    let clouds = 1.0 - night.clamp(0.0, 1.0);
    if clouds > 0.0 {
        for y in 0..(horizon as f32 * 0.6) as i32 {
            for x in 0..c.w {
                let v = noise(x as f32 * 0.07 + t * 0.12, y as f32 * 0.22, 3);
                if v > 0.62 {
                    c.blend(x, y, Rgb(255.0, 255.0, 255.0), clouds * ((v - 0.62) * 4.0).min(0.7));
                }
            }
        }
    }
}

fn sun_and_moon(c: &mut Canvas, horizon: i32, night: f32, day: u32) {
    let (cx, cy) = (c.w as f32 * 0.84, horizon as f32 * 0.32);
    let r = (c.h as f32 * 0.075).max(2.5);
    let sun = (1.0 - night * 1.6).clamp(0.0, 1.0);
    if sun > 0.0 {
        c.halo(cx, cy, r * 3.2, Rgb(255.0, 210.0, 120.0), 0.35 * sun);
        for y in (cy - r) as i32..=(cy + r) as i32 {
            for x in (cx - r) as i32..=(cx + r) as i32 {
                if ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() <= r {
                    c.blend(x, y, Rgb::hex(0xffe9a8), sun);
                }
            }
        }
    }
    let moon = ((night - 0.4) * 1.7).clamp(0.0, 1.0);
    if moon > 0.0 {
        let r = r * 1.25;
        c.halo(cx, cy, r * 3.5, Rgb(150.0, 170.0, 255.0), 0.45 * moon);
        let shadow_dx = (r * 2.3 - (day.saturating_sub(1)) as f32 * r * 0.35).max(r * 0.55);
        for y in (cy - r) as i32 - 1..=(cy + r) as i32 + 1 {
            for x in (cx - r) as i32 - 1..=(cx + r) as i32 + 1 {
                let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
                if d > r {
                    continue;
                }
                let crater = noise(fx * 0.45, fy * 0.45, 17);
                let mut col = Rgb::hex(0xf2f0e6).mix(Rgb::hex(0xb9b8c8), ((crater - 0.45) * 2.5).clamp(0.0, 0.6));
                let ds = ((fx - cx - shadow_dx).powi(2) + (fy - cy).powi(2)).sqrt();
                if ds < r {
                    col = col.scale(0.18);
                }
                c.blend(x, y, col, moon);
            }
        }
    }
}

fn village(c: &mut Canvas, horizon: i32, night: f32, t: f32) {
    let hill = Rgb::hex(0x5d8a5a).mix(Rgb::hex(0x10161f), night);
    for x in 0..c.w {
        let top = horizon - 2 - ((x as f32 * 0.045).sin() * 2.0 + (x as f32 * 0.11).sin() * 1.2 + 1.5) as i32;
        for y in top..horizon {
            c.set(x, y, hill);
        }
    }
    let houses = (c.w / 22).clamp(3, 9);
    for k in 0..houses {
        let jitter = (hash01(k as i64, 1, 5) - 0.5) * 8.0;
        let bx = (c.w as f32 * (k as f32 + 0.5) / houses as f32 + jitter) as i32;
        let w = 9 + (hash01(k as i64, 2, 5) * 5.0) as i32;
        let h = 5 + (hash01(k as i64, 3, 5) * 4.0) as i32;
        let wall = Rgb::hex(0xc9a982).mix(Rgb::hex(0x231f2b), night);
        let roof = Rgb::hex(0x9c4a3a).mix(Rgb::hex(0x161320), night);
        let base = horizon + 1;
        c.fill(bx - w / 2, base - h, w, h, wall);
        let roof_h = w / 2 + 1;
        for i in 0..roof_h {
            c.fill(bx - w / 2 - 1 + i, base - h - i - 1, w + 2 - 2 * i, 1, roof);
        }
        c.fill(bx + w / 4, base - h - roof_h + 1, 2, 3, roof);
        let lit = hash01(k as i64, 4, 5) > 0.25;
        for wx in [bx - w / 4 - 1, bx + w / 4 - 1] {
            let flicker = 0.85 + 0.15 * (t * 3.0 + k as f32 * 1.7).sin();
            let glass = if lit { Rgb::hex(0x5a4a3a).mix(Rgb::hex(0xffc66e).scale(flicker), night) } else { Rgb::hex(0x5a4a3a).mix(Rgb::hex(0x1a1822), night) };
            c.fill(wx, base - h + 2, 2, 2, glass);
            if lit && night > 0.5 {
                c.halo(wx as f32 + 1.0, base as f32 - h as f32 + 3.0, 4.0, Rgb(255.0, 170.0, 80.0), 0.25 * night);
            }
        }
    }
    let grass_top = Rgb::hex(0x6a9a52).mix(Rgb::hex(0x16211f), night);
    let grass_low = Rgb::hex(0x46703a).mix(Rgb::hex(0x0a0f10), night);
    for y in horizon..c.h {
        let row = grass_top.mix(grass_low, (y - horizon) as f32 / (c.h - horizon).max(1) as f32);
        for x in 0..c.w {
            let speck = hash01(x as i64, y as i64, 13);
            c.set(x, y, if speck < 0.06 { row.scale(1.18) } else { row });
        }
    }
}

fn fire(c: &mut Canvas, fx: f32, fy: f32, night: f32, t: f32) {
    let flicker = 0.82 + 0.18 * noise(t * 6.0, 0.0, 21);
    c.halo(fx, fy - 2.0, c.w as f32 * 0.24, Rgb(255.0, 130.0, 50.0), (0.06 + 0.42 * night) * flicker);
    let log = Rgb::hex(0x5b3a22).mix(Rgb::hex(0x3a2416), night);
    c.fill(fx as i32 - 4, fy as i32, 9, 1, log);
    c.fill(fx as i32 - 3, fy as i32 + 1, 7, 1, log.scale(0.8));
    for i in 0..46 {
        let seed = i as f32 * 0.618;
        let phase = (t * (0.9 + (i % 5) as f32 * 0.13) + seed).fract();
        let sway = (seed * 12.9 + t * 3.1).sin() * (1.0 - phase) * 2.6;
        let x = fx + sway + ((i % 3) as f32 - 1.0) * (1.0 - phase) * 1.5;
        let y = fy - 0.5 - phase * 8.0;
        let col = ramp(
            &[(0.0, Rgb::hex(0xfff4c2)), (0.25, Rgb::hex(0xffc23d)), (0.55, Rgb::hex(0xff6a1f)), (0.8, Rgb::hex(0xb32a12)), (1.0, Rgb::hex(0x3a2a2a))],
            phase,
        );
        let alpha = if phase > 0.8 { (1.0 - phase) * 3.0 } else { 1.0 };
        c.blend(x.round() as i32, y.round() as i32, col, alpha.clamp(0.0, 1.0));
    }
    for i in 0..6 {
        let phase = (t * 0.5 + i as f32 * 0.37).fract();
        let x = fx + ((i as f32 * 3.7 + t).sin()) * 4.0 * phase;
        let y = fy - 6.0 - phase * 14.0;
        c.glow(x as i32, y as i32, Rgb(255.0, 150.0, 60.0), (1.0 - phase) * 0.9);
    }
}

fn fog(c: &mut Canvas, horizon: i32, night: f32, t: f32) {
    if night <= 0.05 {
        return;
    }
    for y in horizon - 3..c.h {
        for x in 0..c.w {
            let v = noise(x as f32 * 0.05 - t * 0.18, y as f32 * 0.21, 11);
            let a = (v - 0.48).max(0.0) * 1.3 * night;
            if a > 0.0 {
                c.blend(x, y, Rgb::hex(0x7c86a8), a.min(0.45));
            }
        }
    }
}

pub fn seat_position(i: usize, n: usize, w: i32, h: i32) -> (f32, f32) {
    let (cx, cy) = (w as f32 / 2.0, h as f32 * 0.86);
    let rx = (w as f32 * 0.40).min(h as f32 * 1.9);
    let ry = h as f32 * 0.30;
    let a = PI + PI * (i as f32 + 0.5) / n as f32;
    (cx + a.cos() * rx, cy + a.sin() * ry)
}

impl Scene<'_> {
    fn person(&self, c: &mut Canvas, i: usize, foot_x: f32, foot_y: f32) -> (i32, i32) {
        let seat = &self.seats[i];
        let speaking = self.speaker == Some(i);
        let bob = if speaking && (self.t * 9.0).sin() > 0.0 { 1 } else { 0 };
        let (x0, y0) = (foot_x.round() as i32 - 3, foot_y.round() as i32 - 12 - bob);
        let wolf_form = self.unmasked.get(i).copied().unwrap_or(false) && seat.role == Role::Wolf && self.night > 0.6;
        let tint = Rgb::from(seat.model.rgb());
        let shade = 1.0 - self.night * 0.45;
        let hair = Rgb::hex(HAIR[i % HAIR.len()]).scale(shade);
        let skin = Rgb::hex(SKIN[(i * 3 + 1) % SKIN.len()]).scale(shade);
        let cloak = tint.scale(0.78 * shade + 0.1);
        let fur = Rgb::hex(0x4b4656);
        let eyes = Rgb::hex(0xffd84a);
        let dissolve = seat.dissolve.unwrap_or(0.0);
        let palette = |ch: char| -> Option<Rgb> {
            Some(match ch {
                'h' => hair,
                's' => skin,
                'e' => Rgb::hex(0x1c1820),
                'c' => cloak,
                'b' => Rgb::hex(0x3a2a20),
                'f' => fur,
                'y' => eyes,
                'n' => Rgb::hex(0x6e6878),
                _ => return None,
            })
        };
        let rows: &[&str] = if wolf_form { &WOLF } else { &HUMAN };
        if seat.alive || seat.dissolve.is_some() {
            for (dy, row) in rows.iter().enumerate() {
                for (dx, ch) in row.chars().enumerate() {
                    let (px, py) = (x0 + dx as i32, y0 + dy as i32);
                    if dissolve > 0.0 && hash01(px as i64, py as i64, i as u64 + 31) < dissolve {
                        continue;
                    }
                    if let Some(col) = palette(ch) {
                        c.set(px, py, col);
                    }
                }
            }
            if wolf_form {
                c.halo(x0 as f32 + 1.0, y0 as f32 + 3.0, 3.0, Rgb(255.0, 210.0, 60.0), 0.8);
                c.halo(x0 as f32 + 5.0, y0 as f32 + 3.0, 3.0, Rgb(255.0, 210.0, 60.0), 0.8);
            }
            if dissolve > 0.0 {
                for k in 0..14 {
                    let rise = dissolve * (6.0 + hash01(k, i as i64, 3) * 10.0);
                    let ex = x0 as f32 + 3.0 + (hash01(k, i as i64, 4) - 0.5) * 9.0;
                    let ey = y0 as f32 + 6.0 - rise;
                    c.glow(ex as i32, ey as i32, Rgb(255.0, 140.0, 50.0), (1.0 - dissolve) * 1.2);
                }
            }
        }
        if !seat.alive {
            let alpha = if seat.dissolve.is_some() { dissolve } else { 1.0 };
            let stone = Rgb::hex(0x9a9cab).scale(1.0 - self.night * 0.5);
            let tomb_palette = |ch: char| -> Option<Rgb> {
                Some(match ch {
                    'g' => stone,
                    '+' => stone.scale(0.55),
                    'd' => Rgb::hex(0x4a3628).scale(1.0 - self.night * 0.5),
                    _ => return None,
                })
            };
            for (dy, row) in TOMB.iter().enumerate() {
                for (dx, ch) in row.chars().enumerate() {
                    if let Some(col) = tomb_palette(ch) {
                        c.blend(x0 + dx as i32, foot_y.round() as i32 - 8 + dy as i32, col, alpha);
                    }
                }
            }
        }
        (x0, y0)
    }

    pub fn draw(&self, area: Rect, buf: &mut Buffer) {
        if area.width < 10 || area.height < 6 {
            return;
        }
        let mut c = Canvas::new(area.width, area.height);
        let horizon = (c.h as f32 * 0.42) as i32;
        sky(&mut c, horizon, self.night, self.t);
        sun_and_moon(&mut c, horizon, self.night, self.day);
        village(&mut c, horizon, self.night, self.t);
        let (fx, fy) = (c.w as f32 / 2.0, c.h as f32 * 0.86);
        let n = self.seats.len();
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|a, b| seat_position(*a, n, c.w, c.h).1.total_cmp(&seat_position(*b, n, c.w, c.h).1));
        let mut heads = vec![(0, 0); n];
        for &i in &order {
            let (x, y) = seat_position(i, n, c.w, c.h);
            if self.speaker == Some(i) {
                let tint = Rgb::from(self.seats[i].model.rgb());
                c.halo(x, y - 5.0, 9.0, tint, 0.35 + 0.15 * (self.t * 4.0).sin());
                c.ellipse_ring(x, y + 0.5, 5.0, 1.6, tint, 0.9);
            }
            if let Some((target, col)) = self.mark
                && target == i
            {
                let pulse = 0.6 + 0.4 * (self.t * 6.0).sin();
                c.halo(x, y - 5.0, 10.0, col, 0.55 * pulse);
                c.ellipse_ring(x, y + 0.5, 5.5, 1.8, col, pulse);
            }
            heads[i] = self.person(&mut c, i, x, y);
            if self.thinking == Some(i) {
                let (hx, hy) = heads[i];
                for k in 0..3 {
                    let on = ((self.t * 3.0) as i32 % 4) > k;
                    if on {
                        c.set(hx + 1 + k * 2, hy - 3, Rgb(240.0, 240.0, 250.0));
                    }
                }
            }
        }
        fire(&mut c, fx, fy, self.night, self.t);
        fog(&mut c, horizon, self.night, self.t);
        if let Some((text, alpha)) = &self.banner {
            let scale = if text_width(text, 2) + 4 <= c.w { 2 } else { 1 };
            let tw = text_width(text, scale);
            let (bx, by) = ((c.w - tw) / 2, (horizon as f32 * 0.35) as i32);
            let night = self.night;
            big_text(&mut c, bx + scale, by + scale, text, scale, &|_, _| Rgb(10.0, 8.0, 20.0), alpha * 0.7);
            big_text(
                &mut c,
                bx,
                by,
                text,
                scale,
                &|_, y| {
                    let k = y as f32 / (7.0 * scale as f32);
                    Rgb::hex(0xfff1c9).mix(Rgb::hex(0xff9b4a), k).mix(Rgb::hex(0xdfe6ff).mix(Rgb::hex(0x8a9bff), k), night)
                },
                *alpha,
            );
        }
        if let Some((col, alpha)) = self.flash {
            for y in 0..c.h {
                for x in 0..c.w {
                    c.blend(x, y, col, alpha);
                }
            }
        }
        c.render(area, buf);
        for i in 0..n {
            let (x, y) = seat_position(i, n, c.w, c.h);
            let seat = &self.seats[i];
            let name = &seat.name;
            let row = area.y as i32 + ((y.round() as i32 + 2) / 2).min(area.height as i32 - 1);
            let col = area.x as i32 + x.round() as i32 - name.chars().count() as i32 / 2;
            let fg = if !seat.alive {
                Rgb(130.0, 130.0, 140.0)
            } else if self.speaker == Some(i) {
                Rgb(255.0, 255.0, 255.0)
            } else {
                Rgb::from(seat.model.rgb())
            };
            label(buf, col, row, name, fg, self.speaker == Some(i), area);
        }
    }
}

pub fn title(area: Rect, buf: &mut Buffer, t: f32) -> i32 {
    title_with(area, buf, t, "CCWOLF")
}

pub fn title_with(area: Rect, buf: &mut Buffer, t: f32, text: &str) -> i32 {
    let mut c = Canvas::new(area.width, area.height);
    let horizon = (c.h as f32 * 0.78) as i32;
    sky(&mut c, horizon, 1.0, t);
    let (mx, my, r) = (c.w as f32 * 0.5, c.h as f32 * 0.36, (c.h as f32 * 0.26).min(c.w as f32 * 0.16));
    c.halo(mx, my, r * 2.6, Rgb(140.0, 160.0, 255.0), 0.5);
    for y in (my - r) as i32..=(my + r) as i32 {
        for x in (mx - r) as i32..=(mx + r) as i32 {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            if ((fx - mx).powi(2) + (fy - my).powi(2)).sqrt() <= r {
                let crater = noise(fx * 0.18, fy * 0.18, 17);
                c.set(x, y, Rgb::hex(0xf4f1e4).mix(Rgb::hex(0xa9a8bd), ((crater - 0.42) * 2.2).clamp(0.0, 0.7)));
            }
        }
    }
    for layer in 0..2 {
        let base = if layer == 0 { horizon - 3 } else { horizon + 2 };
        let col = if layer == 0 { Rgb::hex(0x141a2e) } else { Rgb::hex(0x06080f) };
        let mut x = -4;
        let mut k = 0i64;
        while x < c.w + 4 {
            let h = 7.0 + hash01(k, layer, 9) * if layer == 0 { 8.0 } else { 12.0 };
            let half = (h * 0.42) as i32;
            for dy in 0..h as i32 {
                let span = (half as f32 * dy as f32 / h) as i32;
                c.fill(x - span, base - h as i32 + dy, span * 2 + 1, 1, col);
            }
            x += 3 + (hash01(k, layer, 10) * 5.0) as i32;
            k += 1;
        }
        c.fill(0, base, c.w, c.h - base, col);
    }
    for k in 0..7 {
        let ex = (hash01(k, 1, 77) * c.w as f32) as i32;
        let ey = horizon + 3 + (hash01(k, 2, 77) * (c.h - horizon - 5).max(1) as f32) as i32;
        let blink = (t * 0.8 + k as f32 * 2.3).sin();
        if blink > 0.2 && (t * 7.0 + k as f32).sin() > -0.95 {
            let glow = Rgb(255.0, 200.0, 60.0);
            c.set(ex, ey, glow);
            c.set(ex + 3, ey, glow);
            c.halo(ex as f32 + 1.5, ey as f32, 3.0, Rgb(255.0, 160.0, 40.0), 0.35);
        }
    }
    fog(&mut c, horizon - 6, 1.0, t);
    let scale = (1..=3).rev().find(|s| text_width(text, *s) + 6 <= c.w).unwrap_or(1);
    let tw = text_width(text, scale);
    let (tx, ty) = ((c.w - tw) / 2, (my - 3.5 * scale as f32) as i32);
    let ink = |_: i32, _: i32| Rgb(8.0, 5.0, 16.0);
    big_text(&mut c, tx + scale, ty + scale + 1, text, scale, &ink, 0.9);
    for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)] {
        big_text(&mut c, tx + dx, ty + dy, text, scale, &ink, 1.0);
    }
    let shimmer = t;
    big_text(
        &mut c,
        tx,
        ty,
        text,
        scale,
        &|x, y| {
            let k = y as f32 / (7.0 * scale as f32);
            let wave = ((x as f32 * 0.12) - shimmer * 2.2).sin().max(0.0).powi(6) * 0.6;
            ramp(&[(0.0, Rgb::hex(0xffd98a)), (0.45, Rgb::hex(0xff6f4f)), (1.0, Rgb::hex(0xb3204a))], k).mix(Rgb::hex(0xfff6e0), wave)
        },
        1.0,
    );
    c.render(area, buf);
    area.y as i32 + (ty + 7 * scale) / 2 + 1
}
