use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgb(pub f32, pub f32, pub f32);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0.0, 0.0, 0.0);

    pub fn hex(v: u32) -> Rgb {
        Rgb(((v >> 16) & 255) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32)
    }

    pub fn from((r, g, b): (u8, u8, u8)) -> Rgb {
        Rgb(r as f32, g as f32, b as f32)
    }

    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        Rgb(self.0 + (other.0 - self.0) * t, self.1 + (other.1 - self.1) * t, self.2 + (other.2 - self.2) * t)
    }

    pub fn scale(self, k: f32) -> Rgb {
        Rgb(self.0 * k, self.1 * k, self.2 * k)
    }

    pub fn add(self, o: Rgb) -> Rgb {
        Rgb(self.0 + o.0, self.1 + o.1, self.2 + o.2)
    }

    pub fn color(self) -> Color {
        let c = |v: f32| v.round().clamp(0.0, 255.0) as u8;
        Color::Rgb(c(self.0), c(self.1), c(self.2))
    }

    pub fn of(color: Color) -> Option<Rgb> {
        match color {
            Color::Rgb(r, g, b) => Some(Rgb(r as f32, g as f32, b as f32)),
            _ => None,
        }
    }
}

pub fn ramp(stops: &[(f32, Rgb)], t: f32) -> Rgb {
    let t = t.clamp(0.0, 1.0);
    for pair in stops.windows(2) {
        let ((a, ca), (b, cb)) = (pair[0], pair[1]);
        if t <= b {
            return ca.mix(cb, (t - a) / (b - a).max(1e-6));
        }
    }
    stops.last().map(|s| s.1).unwrap_or(Rgb::BLACK)
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    px: Vec<Rgb>,
}

impl Canvas {
    pub fn new(w: u16, rows: u16) -> Canvas {
        let (w, h) = (w as i32, rows as i32 * 2);
        Canvas { w, h, px: vec![Rgb::BLACK; (w * h).max(0) as usize] }
    }

    fn index(&self, x: i32, y: i32) -> Option<usize> {
        (x >= 0 && y >= 0 && x < self.w && y < self.h).then(|| (y * self.w + x) as usize)
    }

    pub fn get(&self, x: i32, y: i32) -> Rgb {
        self.index(x, y).map(|i| self.px[i]).unwrap_or(Rgb::BLACK)
    }

    pub fn set(&mut self, x: i32, y: i32, c: Rgb) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = c;
        }
    }

    pub fn blend(&mut self, x: i32, y: i32, c: Rgb, alpha: f32) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = self.px[i].mix(c, alpha);
        }
    }

    pub fn glow(&mut self, x: i32, y: i32, c: Rgb, k: f32) {
        if let Some(i) = self.index(x, y) {
            self.px[i] = self.px[i].add(c.scale(k));
        }
    }

    pub fn fill(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Rgb) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, c);
            }
        }
    }

    pub fn halo(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, strength: f32) {
        for y in (cy - r).floor() as i32..=(cy + r).ceil() as i32 {
            for x in (cx - r).floor() as i32..=(cx + r).ceil() as i32 {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt() / r;
                if d < 1.0 {
                    self.glow(x, y, c, strength * (1.0 - d).powi(2));
                }
            }
        }
    }

    pub fn ellipse_ring(&mut self, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb, alpha: f32) {
        for y in (cy - ry - 1.0) as i32..=(cy + ry + 1.0) as i32 {
            for x in (cx - rx - 1.0) as i32..=(cx + rx + 1.0) as i32 {
                let d = ((x as f32 - cx) / rx).powi(2) + ((y as f32 - cy) / ry).powi(2);
                if (0.55..=1.0).contains(&d) {
                    self.blend(x, y, c, alpha * (1.0 - (d - 0.78).abs() * 3.0).max(0.0));
                }
            }
        }
    }

    pub fn render(&self, area: Rect, buf: &mut Buffer) {
        for row in 0..area.height as i32 {
            for col in 0..area.width as i32 {
                let (top, bottom) = (self.get(col, row * 2), self.get(col, row * 2 + 1));
                if let Some(cell) = buf.cell_mut((area.x + col as u16, area.y + row as u16)) {
                    cell.set_char('\u{2580}').set_fg(top.color()).set_bg(bottom.color());
                }
            }
        }
    }
}

pub fn label(buf: &mut Buffer, x: i32, y: i32, text: &str, fg: Rgb, bold: bool, area: Rect) {
    for (i, ch) in text.chars().enumerate() {
        let cx = x + i as i32;
        if cx < area.x as i32 || cx >= area.right() as i32 || y < area.y as i32 || y >= area.bottom() as i32 {
            continue;
        }
        if let Some(cell) = buf.cell_mut((cx as u16, y as u16)) {
            let top = Rgb::of(cell.fg).unwrap_or(Rgb::BLACK);
            let bottom = Rgb::of(cell.bg).unwrap_or(Rgb::BLACK);
            let back = if cell.symbol() == "\u{2580}" { top.mix(bottom, 0.5) } else { bottom };
            cell.set_char(ch).set_fg(fg.color()).set_bg(back.scale(0.55).color());
            if bold {
                cell.modifier.insert(ratatui::style::Modifier::BOLD);
            }
        }
    }
}

const GLYPHS: &[(char, [&str; 7])] = &[
    ('A', [" ### ", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"]),
    ('B', ["#### ", "#   #", "#   #", "#### ", "#   #", "#   #", "#### "]),
    ('C', [" ### ", "#   #", "#    ", "#    ", "#    ", "#   #", " ### "]),
    ('D', ["#### ", "#   #", "#   #", "#   #", "#   #", "#   #", "#### "]),
    ('E', ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#####"]),
    ('F', ["#####", "#    ", "#    ", "#### ", "#    ", "#    ", "#    "]),
    ('G', [" ### ", "#   #", "#    ", "# ###", "#   #", "#   #", " ####"]),
    ('H', ["#   #", "#   #", "#   #", "#####", "#   #", "#   #", "#   #"]),
    ('I', [" ### ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "]),
    ('J', ["  ###", "   # ", "   # ", "   # ", "   # ", "#  # ", " ##  "]),
    ('K', ["#   #", "#  # ", "# #  ", "##   ", "# #  ", "#  # ", "#   #"]),
    ('L', ["#    ", "#    ", "#    ", "#    ", "#    ", "#    ", "#####"]),
    ('M', ["#   #", "## ##", "# # #", "# # #", "#   #", "#   #", "#   #"]),
    ('N', ["#   #", "#   #", "##  #", "# # #", "#  ##", "#   #", "#   #"]),
    ('O', [" ### ", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "]),
    ('P', ["#### ", "#   #", "#   #", "#### ", "#    ", "#    ", "#    "]),
    ('Q', [" ### ", "#   #", "#   #", "#   #", "# # #", "#  # ", " ## #"]),
    ('R', ["#### ", "#   #", "#   #", "#### ", "# #  ", "#  # ", "#   #"]),
    ('S', [" ####", "#    ", "#    ", " ### ", "    #", "    #", "#### "]),
    ('T', ["#####", "  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "  #  "]),
    ('U', ["#   #", "#   #", "#   #", "#   #", "#   #", "#   #", " ### "]),
    ('V', ["#   #", "#   #", "#   #", "#   #", "#   #", " # # ", "  #  "]),
    ('W', ["#   #", "#   #", "#   #", "# # #", "# # #", "# # #", " # # "]),
    ('X', ["#   #", "#   #", " # # ", "  #  ", " # # ", "#   #", "#   #"]),
    ('Y', ["#   #", "#   #", " # # ", "  #  ", "  #  ", "  #  ", "  #  "]),
    ('Z', ["#####", "    #", "   # ", "  #  ", " #   ", "#    ", "#####"]),
    ('0', [" ### ", "#   #", "#  ##", "# # #", "##  #", "#   #", " ### "]),
    ('1', ["  #  ", " ##  ", "  #  ", "  #  ", "  #  ", "  #  ", " ### "]),
    ('2', [" ### ", "#   #", "    #", "   # ", "  #  ", " #   ", "#####"]),
    ('3', ["#####", "   # ", "  #  ", "   # ", "    #", "#   #", " ### "]),
    ('4', ["   # ", "  ## ", " # # ", "#  # ", "#####", "   # ", "   # "]),
    ('5', ["#####", "#    ", "#### ", "    #", "    #", "#   #", " ### "]),
    ('6', ["  ## ", " #   ", "#    ", "#### ", "#   #", "#   #", " ### "]),
    ('7', ["#####", "    #", "   # ", "  #  ", " #   ", " #   ", " #   "]),
    ('8', [" ### ", "#   #", "#   #", " ### ", "#   #", "#   #", " ### "]),
    ('9', [" ### ", "#   #", "#   #", " ####", "    #", "   # ", " ##  "]),
    ('!', ["  #  ", "  #  ", "  #  ", "  #  ", "  #  ", "     ", "  #  "]),
    ('\'', ["  #  ", "  #  ", " #   ", "     ", "     ", "     ", "     "]),
    ('-', ["     ", "     ", "     ", " ### ", "     ", "     ", "     "]),
    ('?', [" ### ", "#   #", "    #", "   # ", "  #  ", "     ", "  #  "]),
    (',', ["     ", "     ", "     ", "     ", "     ", "  #  ", " #   "]),
    ('.', ["     ", "     ", "     ", "     ", "     ", "     ", "  #  "]),
    (':', ["     ", "  #  ", "     ", "     ", "     ", "  #  ", "     "]),
];

fn fold(c: char) -> char {
    match c.to_uppercase().next().unwrap_or(c) {
        'É' | 'È' | 'Ê' | 'Ë' => 'E',
        'À' | 'Â' | 'Ä' => 'A',
        'Î' | 'Ï' => 'I',
        'Ô' | 'Ö' => 'O',
        'Ù' | 'Û' | 'Ü' => 'U',
        'Ç' => 'C',
        other => other,
    }
}

const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

pub fn truecolor() -> bool {
    matches!(std::env::var("COLORTERM").as_deref(), Ok("truecolor") | Ok("24bit")) || std::env::var_os("WT_SESSION").is_some()
}

pub fn indexed(r: u8, g: u8, b: u8) -> u8 {
    let level = |v: u8| CUBE.iter().enumerate().min_by_key(|(_, l)| (**l as i32 - v as i32).abs()).map(|(i, _)| i).unwrap_or(0);
    let (ri, gi, bi) = (level(r), level(g), level(b));
    let distance = |(x, y, z): (u8, u8, u8)| {
        let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        d(x, r) + d(y, g) + d(z, b)
    };
    let cube = (CUBE[ri], CUBE[gi], CUBE[bi]);
    let mean = (r as u32 + g as u32 + b as u32) / 3;
    let step = ((mean as i32 - 8) / 10).clamp(0, 23) as u8;
    let grey = 8 + step * 10;
    if distance((grey, grey, grey)) < distance(cube) { 232 + step } else { 16 + 36 * ri as u8 + 6 * gi as u8 + bi as u8 }
}

pub fn to_256_colors(buf: &mut Buffer) {
    let area = buf.area;
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell_mut((x, y)) {
                if let Color::Rgb(r, g, b) = cell.fg {
                    cell.fg = Color::Indexed(indexed(r, g, b));
                }
                if let Color::Rgb(r, g, b) = cell.bg {
                    cell.bg = Color::Indexed(indexed(r, g, b));
                }
            }
        }
    }
}

#[cfg(test)]
pub fn glyph(ch: char) -> Option<[&'static str; 7]> {
    GLYPHS.iter().find(|(g, _)| *g == fold(ch)).map(|(_, rows)| *rows)
}

pub fn text_width(text: &str, scale: i32) -> i32 {
    let n = text.chars().count() as i32;
    (n * 6 - 1).max(0) * scale
}

pub fn big_text(c: &mut Canvas, x0: i32, y0: i32, text: &str, scale: i32, paint: &dyn Fn(i32, i32) -> Rgb, alpha: f32) {
    let mut x = x0;
    for ch in text.chars() {
        if let Some((_, rows)) = GLYPHS.iter().find(|(g, _)| *g == fold(ch)) {
            for (gy, row) in rows.iter().enumerate() {
                for (gx, bit) in row.chars().enumerate() {
                    if bit != '#' {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = x + gx as i32 * scale + sx;
                            let py = y0 + gy as i32 * scale + sy;
                            c.blend(px, py, paint(px - x0, py - y0), alpha);
                        }
                    }
                }
            }
        }
        x += 6 * scale;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_banner_letter_has_a_glyph() {
        for text in [
            "CCWOLF",
            "NUIT 12",
            "NIGHT 3",
            "JOUR 4",
            "DAY 9",
            "LES LOUPS GAGNENT",
            "LE VILLAGE GAGNE",
            "WEREWOLVES WIN",
            "THE VILLAGE WINS",
            "PAUSE",
            "PAUSED",
            "ÉGALITÉ",
        ] {
            for ch in text.chars().filter(|c| *c != ' ') {
                assert!(GLYPHS.iter().any(|(g, _)| *g == fold(ch)), "{ch}");
            }
        }
    }

    #[test]
    fn half_blocks_carry_two_pixels_per_cell() {
        let mut c = Canvas::new(2, 1);
        c.set(0, 0, Rgb(255.0, 0.0, 0.0));
        c.set(0, 1, Rgb(0.0, 0.0, 255.0));
        let area = Rect::new(0, 0, 2, 1);
        let mut buf = Buffer::empty(area);
        c.render(area, &mut buf);
        let cell = buf.cell((0, 0)).unwrap();
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0));
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255));
    }

    #[test]
    fn terminals_without_24_bit_color_get_the_nearest_of_256() {
        assert_eq!(indexed(0, 0, 0), 16);
        assert_eq!(indexed(255, 255, 255), 231);
        assert_eq!(indexed(255, 0, 0), 196);
        assert_eq!(indexed(128, 128, 128), 244);
        assert_eq!(indexed(20, 10, 20), 232);
        let area = Rect::new(0, 0, 1, 1);
        let mut buf = Buffer::empty(area);
        buf[(0, 0)].set_fg(Color::Rgb(255, 0, 0)).set_bg(Color::Reset);
        to_256_colors(&mut buf);
        assert_eq!(buf[(0, 0)].fg, Color::Indexed(196));
        assert_eq!(buf[(0, 0)].bg, Color::Reset);
    }
}
