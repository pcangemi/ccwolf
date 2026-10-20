use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use serde::Deserialize;

use super::canvas::Rgb;
use super::sound::{Mix, Sfx};
use super::video::{Faces, Frame, VideoOptions, rasterize};
use super::{App, GOLD, Options, Source, Stage, scene};
use crate::game::Event;

#[derive(Debug, Deserialize)]
pub struct Storyboard {
    pub shots: Vec<Shot>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Shot {
    Card {
        title: String,
        #[serde(default)]
        subtitle: String,
        seconds: f32,
        #[serde(default)]
        sfx: Option<String>,
    },
    Play {
        from: f32,
        to: f32,
        #[serde(default = "one")]
        speed: f32,
        #[serde(default)]
        caption: String,
        #[serde(default)]
        callouts: Vec<Callout>,
    },
}

fn one() -> f32 {
    1.0
}

#[derive(Debug, Deserialize)]
pub struct Callout {
    pub region: Region,
    pub label: String,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Region {
    Scene,
    Speech,
    Thought,
    Gauge,
    Feed,
    Roster,
}

fn region(r: Region, cols: u16, rows: u16) -> Rect {
    let area = Rect::new(0, 0, cols, rows);
    let [_, scene, panels, bottom] = Layout::vertical([Constraint::Length(1), Constraint::Min(14), Constraint::Length(7), Constraint::Length(10)]).areas(area);
    let [speech, thought] = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(panels);
    let [feed, roster] = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(bottom);
    match r {
        Region::Scene => scene,
        Region::Speech => speech,
        Region::Thought => thought,
        Region::Gauge => Rect::new(speech.x + speech.width * 2 / 5, speech.bottom() - 1, speech.width * 3 / 5, 1),
        Region::Feed => feed,
        Region::Roster => roster,
    }
}

struct Output {
    pipe: std::process::ChildStdin,
    frames: u64,
    fps: u32,
}

impl Output {
    fn time(&self) -> f32 {
        self.frames as f32 / self.fps as f32
    }

    fn push(&mut self, frame: &Frame) -> Result<(), String> {
        self.pipe.write_all(&frame.px).map_err(|e| format!("ffmpeg stopped: {e}"))?;
        self.frames += 1;
        Ok(())
    }
}

fn shade(frame: &mut Frame, alpha: f32) {
    if alpha >= 1.0 {
        return;
    }
    for v in frame.px.iter_mut() {
        *v = (*v as f32 * alpha.max(0.0)) as u8;
    }
}

fn caption(frame: &mut Frame, faces: &mut Faces, text: &str, alpha: f32) {
    if text.is_empty() {
        return;
    }
    let px = 46.0;
    let width = faces.measure(text, px);
    let (pad_x, pad_y) = (34, 18);
    let w = width as usize + pad_x * 2;
    let h = px as usize + pad_y * 2;
    let x0 = frame.w.saturating_sub(w) / 2;
    let y0 = 54;
    for y in y0..y0 + h {
        for x in x0..x0 + w {
            let under = frame.get(x, y);
            frame.put(x, y, under.mix(Rgb(8.0, 7.0, 16.0), 0.82 * alpha));
        }
    }
    frame.rect(x0, y0, 8, h, GOLD.scale(alpha));
    faces.text(frame, x0 + pad_x, y0 + pad_y, text, px, Rgb(250.0, 246.0, 236.0).scale(alpha.max(0.2)));
}

pub struct Grid {
    pub cell_w: usize,
    pub cell_h: usize,
    pub cols: u16,
    pub rows: u16,
}

fn callout(frame: &mut Frame, faces: &mut Faces, c: &Callout, grid: &Grid, alpha: f32) {
    let r = region(c.region, grid.cols, grid.rows);
    let (x0, y0) = (r.x as usize * grid.cell_w, r.y as usize * grid.cell_h);
    let (w, h) = (r.width as usize * grid.cell_w, r.height as usize * grid.cell_h);
    let color = GOLD.scale(alpha);
    let t = 4;
    let (x0, y0) = (x0.saturating_sub(t), y0.saturating_sub(t));
    let (w, h) = (w + 2 * t, h + 2 * t);
    for (x, y, rw, rh) in [(x0, y0, w, t), (x0, y0 + h - t, w, t), (x0, y0, t, h), (x0 + w - t, y0, t, h)] {
        for yy in y..y + rh {
            for xx in x..x + rw {
                let under = frame.get(xx.min(frame.w - 1), yy.min(frame.h - 1));
                frame.put(xx, yy, under.mix(color, alpha));
            }
        }
    }
    let px = 30.0;
    let lw = faces.measure(&c.label, px) as usize + 28;
    let lh = px as usize + 14;
    let below = matches!(c.region, Region::Gauge) || y0 < lh + 2;
    let ly = if below { y0 + h } else { y0 - lh };
    let lx = (x0 + w).saturating_sub(lw).min(frame.w.saturating_sub(lw));
    for yy in ly..ly + lh {
        for xx in lx..lx + lw {
            let under = frame.get(xx.min(frame.w - 1), yy.min(frame.h - 1));
            frame.put(xx, yy, under.mix(GOLD, alpha));
        }
    }
    faces.text(frame, lx + 14, ly + 7, &c.label, px, Rgb(14.0, 10.0, 20.0).mix(GOLD, 1.0 - alpha));
}

fn sfx_for(e: &Event) -> Vec<Sfx> {
    match e {
        Event::Night { .. } => vec![Sfx::Howl],
        Event::Dawn { deaths, .. } if deaths.is_empty() => vec![Sfx::Bell],
        Event::Dawn { .. } => vec![Sfx::Bell, Sfx::Sting],
        Event::Verdict { eliminated: Some(_), .. } => vec![Sfx::Sting],
        Event::Vote { .. } => vec![Sfx::Blip],
        _ => Vec::new(),
    }
}

pub fn render(events: Vec<Event>, opts: Options, video: VideoOptions, storyboard: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(storyboard).map_err(|e| format!("{}: {e}", storyboard.display()))?;
    let board: Storyboard = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", storyboard.display()))?;
    let mut last = 0.0;
    for shot in &board.shots {
        if let Shot::Play { from, to, .. } = shot {
            if *from < last || to <= from {
                return Err(format!("play shots must move forward in time ({from} to {to})"));
            }
            last = *to;
        }
    }
    let mut faces = Faces::load(&video.font, &video.italic_font, video.cell_h)?;
    let (w, h) = (video.cols as usize * video.cell_w, video.rows as usize * video.cell_h);
    let mut frame = Frame { w, h, px: vec![0; w * h * 3] };
    let silent = video.out.with_extension("silent.mp4");
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{w}x{h}"), "-r", &video.fps.to_string(), "-i", "-"])
        .args(["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p"])
        .arg(&silent)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start ffmpeg: {e}"))?;
    let mut out = Output { pipe: ffmpeg.stdin.take().ok_or("no ffmpeg input")?, frames: 0, fps: video.fps };
    let mut cues: Vec<(f32, Sfx)> = Vec::new();

    let mut app = App::new(Source::Replay(Vec::new()), opts);
    let mut terminal = Terminal::new(TestBackend::new(video.cols, video.rows)).map_err(|e| e.to_string())?;
    let dt = 1.0 / video.fps as f32;
    let mut sim = 0.0f32;
    let mut card_buffer = Buffer::empty(Rect::new(0, 0, video.cols, video.rows));
    let grid = Grid { cell_w: video.cell_w, cell_h: video.cell_h, cols: video.cols, rows: video.rows };

    let step = |app: &mut App, sim: &mut f32| -> Vec<Event> {
        if app.stage == Stage::Title && *sim >= video.title_seconds {
            app.stage = Stage::Game;
            app.queue.extend(events.iter().cloned());
        }
        let before: Vec<Event> = app.queue.iter().cloned().collect();
        app.tick(dt);
        *sim += dt;
        let consumed = before.len() - app.queue.len();
        before.into_iter().take(consumed).collect()
    };

    for shot in &board.shots {
        match shot {
            Shot::Card { title, subtitle, seconds, sfx } => {
                if let Some(s) = sfx.as_deref().and_then(Sfx::parse) {
                    cues.push((out.time(), s));
                }
                let count = (seconds * video.fps as f32) as u32;
                for i in 0..count {
                    let t = i as f32 / video.fps as f32;
                    card_buffer.reset();
                    let below = scene::title_with(card_buffer.area, &mut card_buffer, t + 2.0, title);
                    rasterize(&card_buffer, &mut faces, &mut frame, video.cell_w, video.cell_h);
                    if !subtitle.is_empty() {
                        let px = 42.0;
                        let tw = faces.measure(subtitle, px) as usize;
                        let y = (below as usize + 2) * video.cell_h;
                        let (bx, bw) = (w.saturating_sub(tw + 60) / 2, tw + 60);
                        for yy in y.saturating_sub(14)..(y + px as usize + 18).min(h) {
                            for xx in bx..(bx + bw).min(w) {
                                let under = frame.get(xx, yy);
                                frame.put(xx, yy, under.mix(Rgb(8.0, 7.0, 16.0), 0.78));
                            }
                        }
                        faces.text(&mut frame, w.saturating_sub(tw) / 2, y, subtitle, px, Rgb(240.0, 236.0, 250.0));
                    }
                    shade(&mut frame, (t / 0.35).min((seconds - t) / 0.35).min(1.0));
                    out.push(&frame)?;
                }
            }
            Shot::Play { from, to, speed, caption: text, callouts } => {
                while sim < *from {
                    step(&mut app, &mut sim);
                }
                let start = sim;
                let mut credit = 0.0f32;
                while sim < *to {
                    let begun = step(&mut app, &mut sim);
                    credit += 1.0 / speed.max(0.1);
                    for e in &begun {
                        for s in sfx_for(e) {
                            cues.push((out.time(), s));
                        }
                    }
                    while credit >= 1.0 {
                        credit -= 1.0;
                        terminal.draw(|f| app.draw(f)).map_err(|e| e.to_string())?;
                        rasterize(terminal.backend().buffer(), &mut faces, &mut frame, video.cell_w, video.cell_h);
                        let local = (sim - start) / speed;
                        let alpha = (local / 0.4).min(1.0);
                        for c in callouts {
                            callout(&mut frame, &mut faces, c, &grid, alpha);
                        }
                        caption(&mut frame, &mut faces, text, alpha);
                        shade(&mut frame, (local / 0.25).min(((to - sim) / speed) / 0.25).min(1.0));
                        out.push(&frame)?;
                    }
                }
            }
        }
    }
    let total = out.time();
    drop(out);
    if !ffmpeg.wait().map_err(|e| e.to_string())?.success() {
        return Err("ffmpeg failed while encoding the video".into());
    }

    let mut mix = Mix::new(total + 1.0);
    mix.music(0.0, total);
    let mut last_blip = -1.0;
    for (at, s) in cues {
        if s == Sfx::Blip && at - last_blip < 0.12 {
            continue;
        }
        if s == Sfx::Blip {
            last_blip = at;
        }
        mix.sfx(at, s);
    }
    let wav: PathBuf = video.out.with_extension("wav");
    mix.write_wav(&wav)?;
    let status = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-i"])
        .arg(&silent)
        .arg("-i")
        .arg(&wav)
        .args(["-c:v", "copy", "-c:a", "aac", "-b:a", "160k", "-shortest", "-movflags", "+faststart"])
        .arg(&video.out)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed while adding the sound".into());
    }
    let _ = std::fs::remove_file(&silent);
    let _ = std::fs::remove_file(&wav);
    println!("{:.1} s, {}", total, video.out.display());
    Ok(())
}
