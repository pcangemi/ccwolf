use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use fontdue::{Font, FontSettings, Metrics};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::{Buffer, Cell};
use ratatui::style::Modifier;

use super::canvas::Rgb;
use super::{App, BG, Options, Source, TEXT};
use crate::game::Event;

pub struct VideoOptions {
    pub out: PathBuf,
    pub cols: u16,
    pub rows: u16,
    pub fps: u32,
    pub font: PathBuf,
    pub italic_font: PathBuf,
    pub cell_w: usize,
    pub cell_h: usize,
    pub title_seconds: f32,
    pub tail_seconds: f32,
}

struct Faces {
    regular: Font,
    italic: Font,
    px: f32,
    ascent: f32,
    cache: HashMap<(char, bool), (Metrics, Vec<u8>)>,
}

impl Faces {
    fn load(regular: &PathBuf, italic: &PathBuf, cell_h: usize) -> Result<Faces, String> {
        let px = cell_h as f32 * 0.70;
        let open = |path: &PathBuf| -> Result<Font, String> {
            let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
            Font::from_bytes(bytes, FontSettings { scale: px, ..FontSettings::default() }).map_err(|e| format!("{}: {e}", path.display()))
        };
        let regular_font = open(regular)?;
        let italic_font = open(italic).unwrap_or_else(|_| regular_font.clone());
        let lines = regular_font.horizontal_line_metrics(px).ok_or("the font has no horizontal metrics")?;
        let ascent = lines.ascent + ((cell_h as f32) - (lines.ascent - lines.descent)) / 2.0;
        Ok(Faces { regular: regular_font, italic: italic_font, px, ascent, cache: HashMap::new() })
    }

    fn glyph(&mut self, ch: char, italic: bool) -> &(Metrics, Vec<u8>) {
        let (font, px) = (if italic { &self.italic } else { &self.regular }, self.px);
        self.cache.entry((ch, italic)).or_insert_with(|| font.rasterize(ch, px))
    }
}

struct Frame {
    w: usize,
    h: usize,
    px: Vec<u8>,
}

impl Frame {
    fn put(&mut self, x: usize, y: usize, c: Rgb) {
        if x < self.w && y < self.h {
            let i = (y * self.w + x) * 3;
            self.px[i] = c.0.clamp(0.0, 255.0) as u8;
            self.px[i + 1] = c.1.clamp(0.0, 255.0) as u8;
            self.px[i + 2] = c.2.clamp(0.0, 255.0) as u8;
        }
    }

    fn get(&self, x: usize, y: usize) -> Rgb {
        let i = (y * self.w + x) * 3;
        Rgb(self.px[i] as f32, self.px[i + 1] as f32, self.px[i + 2] as f32)
    }

    fn rect(&mut self, x0: usize, y0: usize, w: usize, h: usize, c: Rgb) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.put(x, y, c);
            }
        }
    }
}

fn box_lines(ch: char) -> Option<(bool, bool, bool, bool, bool)> {
    Some(match ch {
        '\u{2500}' => (false, false, true, true, false),
        '\u{2502}' => (true, true, false, false, false),
        '\u{256d}' | '\u{250c}' => (false, true, false, true, false),
        '\u{256e}' | '\u{2510}' => (false, true, true, false, false),
        '\u{2570}' | '\u{2514}' => (true, false, false, true, false),
        '\u{256f}' | '\u{2518}' => (true, false, true, false, false),
        '\u{2550}' => (false, false, true, true, true),
        '\u{2551}' => (true, true, false, false, true),
        '\u{2554}' => (false, true, false, true, true),
        '\u{2557}' => (false, true, true, false, true),
        '\u{255a}' => (true, false, false, true, true),
        '\u{255d}' => (true, false, true, false, true),
        _ => return None,
    })
}

fn draw_cell(frame: &mut Frame, faces: &mut Faces, cell: &Cell, x0: usize, y0: usize, cw: usize, ch: usize) {
    let fg = Rgb::of(cell.fg).unwrap_or(TEXT);
    let bg = Rgb::of(cell.bg).unwrap_or(BG);
    let sym = cell.symbol().chars().next().unwrap_or(' ');
    match sym {
        '\u{2580}' => {
            frame.rect(x0, y0, cw, ch / 2, fg);
            frame.rect(x0, y0 + ch / 2, cw, ch - ch / 2, bg);
            return;
        }
        '\u{2588}' => {
            frame.rect(x0, y0, cw, ch, fg);
            return;
        }
        '\u{2591}' => {
            frame.rect(x0, y0, cw, ch, bg.mix(fg, 0.3));
            return;
        }
        _ => frame.rect(x0, y0, cw, ch, bg),
    }
    if let Some((up, down, left, right, double)) = box_lines(sym) {
        let (mx, my) = ((x0 + cw / 2) as i32, (y0 + ch / 2) as i32);
        let offsets: &[i32] = if double { &[-2, 1] } else { &[0] };
        for &o in offsets {
            let (hx, vy) = ((mx + o).max(0) as usize, (my + o).max(0) as usize);
            if left {
                frame.rect(x0, vy, mx as usize - x0 + 1, 1, fg);
            }
            if right {
                frame.rect(mx as usize, vy, x0 + cw - mx as usize, 1, fg);
            }
            if up {
                frame.rect(hx, y0, 1, my as usize - y0 + 1, fg);
            }
            if down {
                frame.rect(hx, my as usize, 1, y0 + ch - my as usize, fg);
            }
        }
        return;
    }
    if sym == ' ' {
        return;
    }
    let bold = cell.modifier.contains(Modifier::BOLD);
    let italic = cell.modifier.contains(Modifier::ITALIC);
    let ascent = faces.ascent;
    let (metrics, coverage) = faces.glyph(sym, italic).clone();
    let gx = x0 as i32 + metrics.xmin + ((cw as i32 - metrics.advance_width.round() as i32) / 2).max(0);
    let gy = y0 as i32 + ascent.round() as i32 - metrics.height as i32 - metrics.ymin;
    for dx in if bold { 0..2 } else { 0..1 } {
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let a = coverage[row * metrics.width + col] as f32 / 255.0;
                if a <= 0.0 {
                    continue;
                }
                let (x, y) = (gx + col as i32 + dx, gy + row as i32);
                if x < 0 || y < 0 || x as usize >= frame.w || y as usize >= frame.h {
                    continue;
                }
                let under = frame.get(x as usize, y as usize);
                frame.put(x as usize, y as usize, under.mix(fg, a));
            }
        }
    }
    if cell.modifier.contains(Modifier::CROSSED_OUT) {
        frame.rect(x0, y0 + ch / 2, cw, 1, fg);
    }
}

fn rasterize(buf: &Buffer, faces: &mut Faces, frame: &mut Frame, cw: usize, ch: usize) {
    for row in 0..buf.area.height {
        for col in 0..buf.area.width {
            if let Some(cell) = buf.cell((col, row)) {
                draw_cell(frame, faces, cell, col as usize * cw, row as usize * ch, cw, ch);
            }
        }
    }
}

fn label(e: &Event, players: &[crate::game::PlayerInfo]) -> String {
    let name = |p: &usize| players.get(*p).map(|x| x.name.clone()).unwrap_or_default();
    match e {
        Event::Night { day } => format!("night {day}"),
        Event::Dawn { day, .. } => format!("day {day}"),
        Event::WolfTalk { player, .. } => format!("wolf talk {}", name(player)),
        Event::Speech { player, round, .. } => format!("speech {} round {round}", name(player)),
        Event::Vote { player, target, .. } => format!("vote {} -> {}", name(player), name(target)),
        Event::Verdict { eliminated, .. } => format!("verdict {}", eliminated.map(|p| name(&p)).unwrap_or_default()),
        Event::LastWords { player, .. } => format!("last words {}", name(player)),
        Event::SeerLook { target, .. } => format!("seer looks at {}", name(target)),
        Event::Witch { .. } => "witch".into(),
        Event::GameOver { .. } => "game over".into(),
        _ => String::new(),
    }
}

pub fn render(events: Vec<Event>, opts: Options, video: VideoOptions) -> Result<(), String> {
    let players = match events.first() {
        Some(Event::Setup { players, .. }) => players.clone(),
        _ => return Err("not a ccwolf game".into()),
    };
    let mut faces = Faces::load(&video.font, &video.italic_font, video.cell_h)?;
    let (w, h) = (video.cols as usize * video.cell_w, video.rows as usize * video.cell_h);
    let mut frame = Frame { w, h, px: vec![0; w * h * 3] };
    let mut ffmpeg = Command::new("ffmpeg")
        .args(["-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", &format!("{w}x{h}"), "-r", &video.fps.to_string(), "-i", "-"])
        .args(["-c:v", "libx264", "-preset", "medium", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(&video.out)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot start ffmpeg: {e}"))?;
    let mut pipe = ffmpeg.stdin.take().ok_or("no ffmpeg input")?;
    let mut app = App::new(Source::Replay(Vec::new()), opts);
    let mut terminal = Terminal::new(TestBackend::new(video.cols, video.rows)).map_err(|e| e.to_string())?;
    let dt = 1.0 / video.fps as f32;
    let mut timeline = Vec::new();
    let mut seen = 0;
    let mut t = 0.0f32;
    let mut over_at: Option<f32> = None;
    let mut frames = 0u64;
    loop {
        if app.stage == super::Stage::Title && t >= video.title_seconds {
            app.stage = super::Stage::Game;
            app.queue.extend(events.iter().cloned());
        }
        app.tick(dt);
        let started = events.len() - app.queue.len();
        while seen < started.min(events.len()) && app.stage == super::Stage::Game {
            let text = label(&events[seen], &players);
            if !text.is_empty() {
                timeline.push(format!("{:>8.2}s  {text}", t));
            }
            seen += 1;
        }
        terminal.draw(|f| app.draw(f)).map_err(|e| e.to_string())?;
        rasterize(terminal.backend().buffer(), &mut faces, &mut frame, video.cell_w, video.cell_h);
        pipe.write_all(&frame.px).map_err(|e| format!("ffmpeg stopped: {e}"))?;
        frames += 1;
        t += dt;
        if over_at.is_none() && (app.winner.is_some() || app.aborted.is_some()) {
            over_at = Some(t);
        }
        if over_at.is_some_and(|o| t >= o + video.tail_seconds) {
            break;
        }
    }
    drop(pipe);
    let status = ffmpeg.wait().map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("ffmpeg failed".into());
    }
    let timeline_path = video.out.with_extension("timeline.txt");
    std::fs::write(&timeline_path, timeline.join("\n") + "\n").map_err(|e| e.to_string())?;
    println!("{} frames, {:.1} s, {} ({w}x{h}), timeline in {}", frames, t, video.out.display(), timeline_path.display());
    Ok(())
}
