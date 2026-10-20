pub mod canvas;
pub mod player;
#[cfg(feature = "video")]
pub mod promo;
pub mod scene;
#[cfg(feature = "video")]
pub mod sound;
#[cfg(feature = "video")]
pub mod video;

use std::collections::VecDeque;
use std::io;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::crossterm::event::{self, Event as TermEvent, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};

use crate::game::{Action, Audience, Event, PlayerInfo, Role, Task, Team, Verdict};
use crate::human;
use crate::lang::{Lang, Ui};
use crate::model::Model;
use canvas::{Rgb, label};
use player::{Input, Outcome};
use scene::{Scene, Seat};

const CPS: f32 = 38.0;
const BG: Rgb = Rgb(11.0, 11.0, 20.0);
const PANEL: Rgb = Rgb(17.0, 16.0, 30.0);
const TEXT: Rgb = Rgb(226.0, 226.0, 236.0);
const DIM: Rgb = Rgb(128.0, 128.0, 150.0);
const WOLF_RED: Rgb = Rgb(232.0, 72.0, 72.0);
const SEER_VIOLET: Rgb = Rgb(186.0, 128.0, 255.0);
const WITCH_GREEN: Rgb = Rgb(110.0, 220.0, 130.0);
const THOUGHT: Rgb = Rgb(206.0, 180.0, 255.0);
const GOLD: Rgb = Rgb(255.0, 210.0, 90.0);

pub type Launch = Box<dyn FnOnce(Sender<Event>) + Send>;

pub enum Source {
    Live(Launch),
    Replay(Vec<Event>),
}

pub struct Options {
    pub lang: Lang,
    pub crt: bool,
    pub speed: f32,
    pub lineup: Vec<Model>,
    pub replay: bool,
    pub jev: bool,
    pub human: Option<human::Seat>,
}

fn role_color(role: Role) -> Rgb {
    match role {
        Role::Wolf => WOLF_RED,
        Role::Seer => SEER_VIOLET,
        Role::Witch => WITCH_GREEN,
        Role::Villager => Rgb(214.0, 196.0, 160.0),
    }
}

fn fg(c: Rgb) -> Style {
    Style::new().fg(c.color())
}

struct Show {
    start: f32,
    dur: f32,
}

struct Panel {
    title: String,
    who: Option<usize>,
    say: String,
    thought: String,
    accent: Rgb,
    start: f32,
    verdict: Option<Verdict>,
}

struct Banner {
    text: String,
    start: f32,
    dur: f32,
}

#[derive(PartialEq)]
enum Stage {
    Title,
    Game,
}

struct App {
    lang: Lang,
    ui: &'static Ui,
    stage: Stage,
    source: Option<Source>,
    rx: Option<Receiver<Event>>,
    queue: VecDeque<Event>,
    players: Vec<PlayerInfo>,
    alive: Vec<bool>,
    dissolve_start: Vec<Option<f32>>,
    night: f32,
    night_target: f32,
    day: u32,
    clock: f32,
    anim: f32,
    show: Option<Show>,
    panel: Option<Panel>,
    speaker: Option<usize>,
    feed: Vec<(String, Rgb)>,
    tally: Vec<u32>,
    voting: bool,
    thinking: Option<usize>,
    mark: Option<(usize, Rgb)>,
    banner: Option<Banner>,
    flash: Option<(Rgb, f32)>,
    shake_until: f32,
    cost: f64,
    winner: Option<Team>,
    aborted: Option<String>,
    show_thoughts: bool,
    show_roles: bool,
    crt: bool,
    paused: bool,
    speed: f32,
    replay: bool,
    lineup: Vec<Model>,
    jev: bool,
    quit: bool,
    confirm_quit: bool,
    me: Option<usize>,
    seat: Option<human::Seat>,
    pending: Option<Task>,
    input: Option<Input>,
    intro: bool,
    visions: Vec<(usize, bool)>,
    potions: (bool, bool),
    my_lies: Vec<f32>,
}

impl App {
    fn new(source: Source, opts: Options) -> App {
        App {
            lang: opts.lang,
            ui: opts.lang.ui(),
            stage: Stage::Title,
            source: Some(source),
            rx: None,
            queue: VecDeque::new(),
            players: Vec::new(),
            alive: Vec::new(),
            dissolve_start: Vec::new(),
            night: 0.0,
            night_target: 0.0,
            day: 0,
            clock: 0.0,
            anim: 0.0,
            show: None,
            panel: None,
            speaker: None,
            feed: Vec::new(),
            tally: Vec::new(),
            voting: false,
            thinking: None,
            mark: None,
            banner: None,
            flash: None,
            shake_until: 0.0,
            cost: 0.0,
            winner: None,
            aborted: None,
            show_thoughts: true,
            show_roles: true,
            crt: opts.crt,
            paused: false,
            speed: opts.speed,
            replay: opts.replay,
            lineup: opts.lineup,
            jev: opts.jev,
            quit: false,
            confirm_quit: false,
            me: opts.human.as_ref().map(|h| h.seat),
            intro: opts.human.is_some(),
            seat: opts.human,
            pending: None,
            input: None,
            visions: Vec::new(),
            potions: (true, true),
            my_lies: Vec::new(),
        }
    }

    fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        match self.source.take() {
            Some(Source::Live(launch)) => {
                std::thread::spawn(move || launch(tx));
            }
            Some(Source::Replay(events)) => {
                std::thread::spawn(move || {
                    for e in events {
                        if tx.send(e).is_err() {
                            break;
                        }
                    }
                });
            }
            None => {}
        }
        self.rx = Some(rx);
        self.stage = Stage::Game;
        self.clock = 0.0;
        if self.me.is_some() {
            self.night = 1.0;
            self.night_target = 1.0;
        }
    }

    fn pump(&mut self) {
        if let Some(rx) = &self.rx {
            while let Ok(e) = rx.try_recv() {
                self.queue.push_back(e);
            }
        }
    }

    fn sees_all(&self) -> bool {
        match self.me {
            None => true,
            Some(me) => self.winner.is_some() || self.aborted.is_some() || !self.alive.get(me).copied().unwrap_or(true),
        }
    }

    fn leave(&mut self) {
        let at_stake = self.me.is_some() && self.stage == Stage::Game && self.winner.is_none() && self.aborted.is_none();
        if at_stake {
            self.confirm_quit = true;
        } else {
            self.quit = true;
        }
    }

    fn role_visible(&self, p: usize) -> bool {
        if self.sees_all() {
            return self.show_roles || !self.alive.get(p).copied().unwrap_or(true);
        }
        let me = self.me.unwrap_or(usize::MAX);
        let both_wolves = self.players.get(me).is_some_and(|m| m.role == Role::Wolf) && self.players[p].role == Role::Wolf;
        p == me || both_wolves || !self.alive.get(p).copied().unwrap_or(true)
    }

    fn model_label(&self, model: Model) -> &'static str {
        if model == Model::Human { self.ui.you } else { model.label() }
    }

    fn answer(&mut self, action: Action) {
        if let Some(seat) = &self.seat {
            let _ = seat.answers.send(action);
        }
        self.pending = None;
        self.input = None;
    }

    fn poll_player(&mut self) {
        if let Some(seat) = &self.seat {
            while let Ok(request) = seat.requests.try_recv() {
                self.pending = Some(request.task);
            }
        }
        let ready = self.show.is_none() && self.queue.is_empty() && !self.intro && self.input.is_none();
        if let (true, Some(task)) = (ready, self.pending.clone()) {
            match Input::from_task(&task) {
                Ok(input) => {
                    self.input = Some(input);
                    self.thinking = None;
                }
                Err(action) => self.answer(action),
            }
        }
    }

    fn name(&self, p: usize) -> String {
        self.players.get(p).map(|x| x.name.clone()).unwrap_or_default()
    }

    fn log(&mut self, text: String, color: Rgb) {
        self.feed.push((text, color));
        if self.feed.len() > 200 {
            self.feed.remove(0);
        }
    }

    fn typing_time(&self, say: &str, thought: &str) -> f32 {
        let longest = if self.show_thoughts { say.chars().count().max(thought.chars().count()) } else { say.chars().count() };
        longest as f32 / CPS + 2.2
    }

    fn open_panel(&mut self, title: &str, who: Option<usize>, say: String, thought: String, accent: Rgb) -> f32 {
        let dur = self.typing_time(&say, &thought);
        self.panel = Some(Panel { title: title.to_string(), who, say, thought, accent, start: self.clock, verdict: None });
        dur
    }

    fn attach(&mut self, verdict: Option<Verdict>, dur: f32) -> f32 {
        let Some(v) = verdict else { return dur };
        if let Some(target) = v.accuses {
            self.mark = Some((target, Rgb(255.0, 150.0, 60.0)));
        }
        if let Some(p) = &mut self.panel {
            p.verdict = Some(v);
        }
        dur + 1.2
    }

    fn banner(&mut self, text: String, dur: f32) {
        self.banner = Some(Banner { text, start: self.clock, dur });
    }

    fn kill(&mut self, p: usize) {
        if let Some(alive) = self.alive.get_mut(p) {
            *alive = false;
            self.dissolve_start[p] = Some(self.clock);
        }
    }

    fn begin(&mut self, e: Event) -> f32 {
        let ui = self.ui;
        let fr = self.lang == Lang::Fr;
        if let Some(me) = self.me {
            match &e {
                Event::SeerLook { player, target, wolf, .. } if *player == me => self.visions.push((*target, *wolf)),
                Event::Witch { player, save, poison, .. } if *player == me => {
                    self.potions.0 &= !*save;
                    self.potions.1 &= poison.is_none();
                }
                Event::Speech { player, verdict: Some(v), .. } if *player == me => self.my_lies.push(v.lie),
                _ => {}
            }
            let secret = matches!(e.audience(), Audience::Wolves | Audience::Only(_));
            if secret && !self.players.is_empty() && !self.sees_all() && !e.visible_to(me, &self.players) {
                return 0.0;
            }
            let dies = match &e {
                Event::Dawn { deaths, .. } => deaths.contains(&me),
                Event::Verdict { eliminated, .. } => *eliminated == Some(me),
                _ => false,
            };
            if dies {
                self.log(ui.now_spectator.to_string(), THOUGHT);
            }
        }
        match e {
            Event::Setup { players, .. } => {
                self.alive = vec![true; players.len()];
                self.dissolve_start = vec![None; players.len()];
                self.tally = vec![0; players.len()];
                self.players = players;
                1.0
            }
            Event::Night { day } => {
                self.day = day;
                self.night_target = 1.0;
                self.voting = false;
                self.mark = None;
                self.panel = None;
                self.speaker = None;
                self.banner(format!("{} {day}", ui.night), 3.0);
                self.log(format!("{} {day}", ui.night), Rgb(140.0, 150.0, 230.0));
                3.4
            }
            Event::WolfTalk { player, thought, say } => {
                self.thinking = None;
                self.speaker = Some(player);
                self.log(format!("{} : {}", self.name(player), say), WOLF_RED.scale(0.85));
                self.open_panel(ui.wolf_council, Some(player), say, thought, WOLF_RED)
            }
            Event::WolfPick { player, target, .. } => {
                self.speaker = Some(player);
                self.mark = Some((target, WOLF_RED));
                let line = if fr {
                    format!("{} veut dévorer {}", self.name(player), self.name(target))
                } else {
                    format!("{} wants to devour {}", self.name(player), self.name(target))
                };
                self.log(line, WOLF_RED);
                1.3
            }
            Event::Pack { target } => {
                self.speaker = None;
                self.mark = Some((target, WOLF_RED));
                let line = if fr { format!("La meute a choisi {}", self.name(target)) } else { format!("The pack chose {}", self.name(target)) };
                self.log(line.clone(), WOLF_RED);
                self.open_panel(ui.wolf_council, None, line, String::new(), WOLF_RED).min(2.2)
            }
            Event::SeerLook { player, target, wolf, thought } => {
                self.speaker = Some(player);
                self.mark = Some((target, SEER_VIOLET));
                let verdict = match (fr, wolf) {
                    (true, true) => format!("{} est un LOUP-GAROU !", self.name(target)),
                    (true, false) => format!("{} n'est pas un loup.", self.name(target)),
                    (false, true) => format!("{} is a WEREWOLF!", self.name(target)),
                    (false, false) => format!("{} is not a werewolf.", self.name(target)),
                };
                self.log(format!("{} : {verdict}", ui.seer), SEER_VIOLET);
                self.open_panel(ui.seer_vision, Some(player), verdict, thought, SEER_VIOLET)
            }
            Event::Witch { player, victim, save, poison, thought } => {
                self.speaker = Some(player);
                let mut parts = Vec::new();
                if let (true, Some(v)) = (save, victim) {
                    self.mark = Some((v, WITCH_GREEN));
                    parts.push(if fr { format!("sauve {}", self.name(v)) } else { format!("saves {}", self.name(v)) });
                }
                if let Some(p) = poison {
                    self.mark = Some((p, SEER_VIOLET));
                    parts.push(if fr { format!("empoisonne {}", self.name(p)) } else { format!("poisons {}", self.name(p)) });
                }
                let text = if parts.is_empty() {
                    if fr { "garde ses potions.".to_string() } else { "keeps the potions.".to_string() }
                } else {
                    format!("{}.", parts.join(if fr { " et " } else { " and " }))
                };
                let text = format!("{} {}", self.name(player), text);
                self.log(format!("{} : {text}", ui.witch), WITCH_GREEN);
                self.open_panel(ui.witch_brew, Some(player), text, thought, WITCH_GREEN)
            }
            Event::Dawn { day, deaths } => {
                self.night_target = 0.0;
                self.mark = None;
                self.speaker = None;
                self.panel = None;
                self.tally = vec![0; self.players.len()];
                self.voting = false;
                self.banner(format!("{} {day}", ui.day), 3.2);
                if deaths.is_empty() {
                    self.log(if fr { "Personne n'est mort cette nuit.".into() } else { "Nobody died last night.".into() }, Rgb(240.0, 220.0, 150.0));
                }
                for d in deaths {
                    self.kill(d);
                    self.flash = Some((Rgb(180.0, 20.0, 30.0), self.clock + 1.2));
                    self.shake_until = self.clock + 1.6;
                    let role = ui.role(self.players[d].role);
                    let line =
                        if fr { format!("Victime de la nuit : {} ({role})", self.name(d)) } else { format!("Victim of the night: {} ({role})", self.name(d)) };
                    self.log(line, Rgb(255.0, 120.0, 110.0));
                }
                3.8
            }
            Event::Speech { player, round, thought, say, verdict } => {
                self.thinking = None;
                self.speaker = Some(player);
                self.mark = None;
                self.log(format!("{} : {}", self.name(player), say), Rgb::from(self.players[player].model.rgb()).mix(TEXT, 0.4));
                let title = format!("{} {}", ui.village_square, if round == 2 { "· 2" } else { "" });
                let dur = self.open_panel(title.trim_end(), Some(player), say, thought, Rgb::from(self.players[player].model.rgb()));
                if Some(player) == self.me {
                    if let Some(p) = &mut self.panel {
                        p.start -= 1000.0;
                    }
                    return 1.5;
                }
                let verdict = if self.sees_all() { verdict } else { None };
                self.attach(verdict, dur)
            }
            Event::Vote { player, target, thought } => {
                self.voting = true;
                self.speaker = Some(player);
                self.mark = Some((target, GOLD));
                if let Some(t) = self.tally.get_mut(target) {
                    *t += 1;
                }
                self.log(format!("{} \u{2192} {}", self.name(player), self.name(target)), GOLD);
                let text = if fr {
                    format!("{} vote contre {}", self.name(player), self.name(target))
                } else {
                    format!("{} votes against {}", self.name(player), self.name(target))
                };
                (self.open_panel(ui.vote, Some(player), text, thought, GOLD) - 1.4).max(1.0)
            }
            Event::Verdict { eliminated, .. } => {
                self.speaker = None;
                match eliminated {
                    Some(p) => {
                        self.mark = Some((p, WOLF_RED));
                        self.kill(p);
                        self.flash = Some((Rgb(255.0, 255.0, 255.0), self.clock + 0.9));
                        self.shake_until = self.clock + 1.4;
                        let role = ui.role(self.players[p].role);
                        let line = if fr {
                            format!("Le village élimine {} ({role})", self.name(p))
                        } else {
                            format!("The village eliminates {} ({role})", self.name(p))
                        };
                        self.log(line, Rgb(255.0, 150.0, 90.0));
                    }
                    None => {
                        self.mark = None;
                        self.log(if fr { "Égalité : personne n'est éliminé.".into() } else { "Tie: nobody is eliminated.".into() }, GOLD);
                    }
                }
                3.0
            }
            Event::LastWords { player, thought, say, verdict } => {
                self.speaker = Some(player);
                self.log(format!("{} : {}", self.name(player), say), DIM);
                let dur = self.open_panel(ui.last_words, Some(player), say, thought, Rgb(200.0, 200.0, 210.0));
                let verdict = if self.sees_all() { verdict } else { None };
                self.attach(verdict, dur)
            }
            Event::Thinking { player } => {
                self.thinking = Some(player);
                self.speaker = None;
                0.0
            }
            Event::Usage { cost, .. } => {
                self.cost += cost;
                0.0
            }
            Event::GameOver { winner } => {
                self.winner = Some(winner);
                self.speaker = None;
                self.mark = None;
                self.thinking = None;
                self.night_target = if winner == Team::Wolves { 0.85 } else { 0.0 };
                self.banner(ui.winner(winner).to_string(), f32::INFINITY);
                self.log(ui.winner(winner).to_string(), Rgb(255.0, 230.0, 140.0));
                f32::INFINITY
            }
            Event::Aborted { reason } => {
                self.thinking = None;
                self.aborted = Some(reason);
                f32::INFINITY
            }
        }
    }

    fn advance(&mut self) {
        if let Some(show) = &self.show {
            if self.clock < show.start + show.dur {
                return;
            }
            self.show = None;
        }
        while self.show.is_none() {
            let Some(e) = self.queue.pop_front() else {
                break;
            };
            let dur = self.begin(e);
            if dur > 0.0 {
                self.show = Some(Show { start: self.clock, dur });
            }
        }
    }

    fn skip(&mut self) {
        let typed_all = self.panel.as_ref().is_none_or(|p| {
            let n = ((self.clock - p.start) * CPS) as usize;
            n >= p.say.chars().count() && (!self.show_thoughts || n >= p.thought.chars().count())
        });
        if !typed_all {
            if let Some(p) = &mut self.panel {
                p.start -= 1000.0;
            }
            return;
        }
        if let Some(show) = &mut self.show
            && show.dur.is_finite()
        {
            show.dur = 0.0;
        }
    }

    fn tick(&mut self, dt: f32) {
        self.anim += dt;
        if !self.paused && !self.intro && self.stage == Stage::Game {
            self.clock += dt * self.speed;
        }
        let step = (dt * self.speed.max(0.5) * 0.9).min(1.0);
        self.night += (self.night_target - self.night) * step;
        self.pump();
        if !self.paused && (!self.intro || self.players.is_empty()) {
            self.advance();
        }
        self.poll_player();
    }

    fn key(&mut self, code: KeyCode) {
        if self.confirm_quit {
            match code {
                KeyCode::Enter | KeyCode::Char('q') | KeyCode::Char('o') | KeyCode::Char('y') => self.quit = true,
                _ => self.confirm_quit = false,
            }
            return;
        }
        if self.stage == Stage::Game && self.intro {
            match code {
                KeyCode::Enter | KeyCode::Char(' ') => self.intro = false,
                KeyCode::Esc => self.leave(),
                _ => {}
            }
            return;
        }
        if let Some(input) = &mut self.input {
            if code == KeyCode::Esc {
                self.leave();
                return;
            }
            match input.key(code) {
                Outcome::Waiting => {}
                Outcome::Next(next) => self.input = Some(next),
                Outcome::Done(action) => self.answer(action),
            }
            return;
        }
        match (&self.stage, code) {
            (_, KeyCode::Char('q')) | (_, KeyCode::Esc) => self.leave(),
            (Stage::Title, KeyCode::Enter) | (Stage::Title, KeyCode::Char(' ')) => self.start(),
            (Stage::Game, KeyCode::Char(' ')) => self.paused = !self.paused,
            (Stage::Game, KeyCode::Right) | (Stage::Game, KeyCode::Enter) => self.skip(),
            (_, KeyCode::Char('t')) => self.show_thoughts = !self.show_thoughts,
            (_, KeyCode::Char('r')) => self.show_roles = !self.show_roles,
            (_, KeyCode::Char('c')) => self.crt = !self.crt,
            (_, KeyCode::Char('+')) | (_, KeyCode::Char('=')) => self.speed = (self.speed * 1.5).min(6.0),
            (_, KeyCode::Char('-')) => self.speed = (self.speed / 1.5).max(0.4),
            _ => {}
        }
    }

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        f.render_widget(Block::new().style(Style::new().bg(BG.color())), area);
        if area.width < 100 || area.height < 30 {
            let msg = Paragraph::new(self.ui.too_small).alignment(Alignment::Center).style(fg(TEXT));
            f.render_widget(msg, Rect::new(area.x, area.y + area.height / 2, area.width, 1));
            return;
        }
        match self.stage {
            Stage::Title => self.draw_title(f, area),
            Stage::Game => self.draw_game(f, area),
        }
        if self.crt {
            crt(f.buffer_mut(), area);
        }
    }

    fn draw_title(&self, f: &mut Frame, area: Rect) {
        let below = scene::title(area, f.buffer_mut(), self.anim);
        let buf = f.buffer_mut();
        let center = |s: &str| area.x as i32 + (area.width as i32 - s.chars().count() as i32) / 2;
        let subtitle = self.ui.subtitle;
        label(buf, center(subtitle), below + 1, subtitle, Rgb(230.0, 230.0, 245.0), false, area);
        let mut line = format!("{} :", self.ui.lineup_hint);
        let mut parts = Vec::new();
        for model in Model::ALL {
            let n = self.lineup.iter().filter(|m| **m == model).count();
            if n > 0 {
                parts.push((format!(" {n} x {}", model.label()), Rgb::from(model.rgb())));
                line.push_str(&format!(" {n} x {}", model.label()));
            }
        }
        if self.me.is_some() {
            let text = format!("{} : {}", self.ui.lineup_hint, self.ui.play_lineup);
            label(buf, center(&text), below + 3, &text, GOLD, true, area);
        } else if !self.replay {
            let mut x = center(&line);
            let head = format!("{} :", self.ui.lineup_hint);
            label(buf, x, below + 3, &head, DIM, false, area);
            x += head.chars().count() as i32;
            for (text, color) in parts {
                label(buf, x, below + 3, &text, color, true, area);
                x += text.chars().count() as i32;
            }
        } else {
            let tag = format!("[ {} ]", self.ui.replay_tag);
            label(buf, center(&tag), below + 3, &tag, Rgb(255.0, 200.0, 90.0), true, area);
        }
        if self.jev && !self.replay {
            let text = self.ui.jev_on;
            label(buf, center(text), below + 4, text, Rgb(255.0, 150.0, 60.0), false, area);
        }
        if (self.anim * 1.6).sin() > -0.3 {
            let press = self.ui.press_enter;
            label(buf, center(press), below + 6, press, Rgb(255.0, 214.0, 120.0), true, area);
        }
        let keys = self.ui.keys;
        label(buf, center(keys), area.bottom() as i32 - 1, keys, DIM, false, area);
    }

    fn draw_game(&mut self, f: &mut Frame, area: Rect) {
        let [top, scene_area, panel_area, bottom] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(14), Constraint::Length(7), Constraint::Length(10)]).areas(area);
        self.draw_top(f, top);

        let seats: Vec<Seat> = (0..self.players.len())
            .map(|i| Seat {
                name: self.players[i].name.clone(),
                model: self.players[i].model,
                role: self.players[i].role,
                alive: self.alive[i],
                dissolve: self.dissolve_start[i].and_then(|s| {
                    let p = (self.clock - s) / 1.8;
                    (p < 1.0).then_some(p.max(0.001))
                }),
            })
            .collect();
        let banner = self.banner.as_ref().and_then(|b| {
            let age = self.clock - b.start;
            let alpha = if b.dur.is_infinite() {
                (age / 0.6).min(1.0)
            } else if age > b.dur {
                0.0
            } else {
                (age / 0.35).min(1.0).min((b.dur - age) / 0.6)
            };
            (alpha > 0.0).then(|| (b.text.clone(), alpha.clamp(0.0, 1.0)))
        });
        let flash = self.flash.and_then(|(c, until)| {
            let left = until - self.clock;
            (left > 0.0).then_some((c, (left / 1.2).min(1.0) * 0.45))
        });
        let shaken = if self.clock < self.shake_until {
            let dx: i32 = if (self.anim * 40.0) as i32 % 2 == 0 { 1 } else { -1 };
            Rect { x: (scene_area.x as i32 + dx).max(0) as u16, ..scene_area }.intersection(scene_area)
        } else {
            scene_area
        };
        let unmasked: Vec<bool> = (0..self.players.len()).map(|i| self.role_visible(i)).collect();
        let banner = if self.intro { self.me.and_then(|me| self.players.get(me)).map(|p| (self.ui.role(p.role).to_uppercase(), 1.0)) } else { banner };
        let mark = match &self.input {
            Some(input) => input.highlighted().map(|p| (p, GOLD)),
            None => self.mark,
        };
        let speaker = if self.input.is_some() { self.me } else { self.speaker };
        let scene = Scene { seats: &seats, night: self.night, day: self.day, t: self.anim, speaker, thinking: self.thinking, mark, unmasked, banner, flash };
        scene.draw(shaken, f.buffer_mut());
        if let Some(p) = self.thinking.filter(|p| Some(*p) != self.me) {
            let text = format!("{} {}...", self.name(p), self.ui.thinking);
            label(f.buffer_mut(), scene_area.x as i32 + 2, scene_area.bottom() as i32 - 1, &text, DIM.mix(TEXT, 0.4), false, scene_area);
        }
        if self.paused {
            let text = format!("  {}  ", self.ui.paused);
            let x = scene_area.x as i32 + (scene_area.width as i32 - text.len() as i32) / 2;
            label(f.buffer_mut(), x, scene_area.y as i32 + 1, &text, Rgb(255.0, 230.0, 140.0), true, scene_area);
        }

        let [speech_area, thought_area] = Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(panel_area);
        self.draw_panels(f, speech_area, thought_area);
        let [feed_area, roster_area] = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(bottom);
        self.draw_feed(f, feed_area);
        self.draw_roster(f, roster_area);
        if self.winner.is_some() || self.aborted.is_some() {
            self.draw_recap(f, scene_area);
        } else if self.intro {
            self.draw_intro(f, scene_area);
        }
        if self.confirm_quit {
            self.draw_quit(f, scene_area);
        }
    }

    fn draw_quit(&self, f: &mut Frame, scene: Rect) {
        let w = 50.min(scene.width.saturating_sub(4));
        let h = 4.min(scene.height);
        let area = Rect::new(scene.x + (scene.width - w) / 2, scene.y + scene.height.saturating_sub(h) / 2, w, h);
        f.render_widget(Clear, area);
        let block = Block::bordered()
            .border_type(BorderType::Double)
            .border_style(fg(GOLD))
            .title(Span::styled(format!(" {} ", self.ui.quit_question), fg(GOLD).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .style(Style::new().bg(Rgb(8.0, 8.0, 16.0).color()));
        let lines = vec![Line::from(""), Line::from(Span::styled(self.ui.quit_hint, fg(TEXT)))];
        f.render_widget(Paragraph::new(lines).alignment(Alignment::Center).block(block), area);
    }

    fn role_card(&self) -> Vec<Line<'static>> {
        let Some(me) = self.me else { return Vec::new() };
        let Some(info) = self.players.get(me) else { return Vec::new() };
        let role_index = match info.role {
            Role::Wolf => 0,
            Role::Seer => 1,
            Role::Witch => 2,
            Role::Villager => 3,
        };
        let mut lines = vec![
            Line::from(vec![
                Span::styled(format!("{} {}, ", self.ui.you_are, info.name), fg(TEXT).add_modifier(Modifier::BOLD)),
                Span::styled(info.persona.clone(), fg(DIM)),
            ]),
            Line::from(Span::styled(self.ui.role(info.role).to_uppercase(), fg(role_color(info.role)).add_modifier(Modifier::BOLD))),
            Line::from(Span::styled(self.ui.role_help[role_index], fg(TEXT))),
        ];
        match info.role {
            Role::Wolf => {
                let pack: Vec<String> = (0..self.players.len()).filter(|&p| p != me && self.players[p].role == Role::Wolf).map(|p| self.name(p)).collect();
                lines.push(Line::from(Span::styled(format!("{} : {}", self.ui.pack, pack.join(", ")), fg(WOLF_RED))));
            }
            Role::Seer if !self.visions.is_empty() => {
                let seen: Vec<String> =
                    self.visions.iter().map(|(p, wolf)| format!("{} {}", self.name(*p), if *wolf { "\u{2620}" } else { "\u{2713}" })).collect();
                lines.push(Line::from(Span::styled(format!("{} : {}", self.ui.visions, seen.join("  ")), fg(SEER_VIOLET))));
            }
            Role::Witch => {
                let left = format!(
                    "{} : {} {}",
                    self.ui.potions_left,
                    if self.potions.0 { "\u{2665}" } else { "\u{00b7}" },
                    if self.potions.1 { "\u{2620}" } else { "\u{00b7}" }
                );
                lines.push(Line::from(Span::styled(left, fg(WITCH_GREEN))));
            }
            _ => {}
        }
        lines
    }

    fn draw_intro(&self, f: &mut Frame, scene: Rect) {
        let w = 76.min(scene.width.saturating_sub(4));
        let h = 9.min(scene.height.saturating_sub(2));
        let area = Rect::new(scene.x + (scene.width - w) / 2, scene.y + scene.height.saturating_sub(h) * 2 / 3, w, h);
        f.render_widget(Clear, area);
        let accent = self.me.and_then(|me| self.players.get(me)).map(|p| role_color(p.role)).unwrap_or(GOLD);
        let block = Block::bordered()
            .border_type(BorderType::Double)
            .border_style(fg(accent))
            .title(Span::styled(format!(" {} ", self.ui.your_role), fg(accent).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .style(Style::new().bg(Rgb(8.0, 8.0, 16.0).color()));
        let mut lines = self.role_card();
        lines.push(Line::from(""));
        if (self.anim * 1.6).sin() > -0.3 {
            lines.push(Line::from(Span::styled(self.ui.start_hint, fg(GOLD).add_modifier(Modifier::BOLD))));
        }
        f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: true }), area);
    }

    fn draw_input(&self, f: &mut Frame, area: Rect, input: &Input) {
        let victim = match input {
            Input::Save { victim, .. } => self.name(*victim),
            _ => String::new(),
        };
        let block = Block::bordered()
            .border_type(BorderType::Double)
            .border_style(fg(GOLD))
            .title(Span::styled(format!(" {} ", input.title(self.lang, &victim)), fg(GOLD).add_modifier(Modifier::BOLD)))
            .title_bottom(Line::from(Span::styled(format!(" {} ", input.hint(self.lang)), fg(DIM))).alignment(Alignment::Right))
            .style(Style::new().bg(PANEL.color()));
        let inner = block.inner(area);
        let lines: Vec<Line> = match input {
            Input::Speech { buffer, .. } => {
                let cursor = if (self.anim * 3.0) as i32 % 2 == 0 { "\u{2588}" } else { " " };
                let wrapped = wrap(&format!("{buffer}{cursor}"), inner.width as usize);
                let skip = wrapped.len().saturating_sub(inner.height as usize);
                wrapped.into_iter().skip(skip).map(|l| Line::from(Span::styled(l, fg(TEXT)))).collect()
            }
            Input::Pick { options, index, .. } | Input::Poison { options, index, .. } => {
                let mut spans = Vec::new();
                for (i, p) in options.iter().enumerate() {
                    let name = if *p == player::NOBODY { crate::prompts::nobody(self.lang).to_string() } else { self.name(*p) };
                    let style = if i == *index { Style::new().fg(BG.color()).bg(GOLD.color()).add_modifier(Modifier::BOLD) } else { fg(TEXT) };
                    spans.push(Span::styled(format!(" {name} "), style));
                    spans.push(Span::raw("  "));
                }
                vec![Line::from(""), Line::from(spans)]
            }
            Input::Save { index, .. } => {
                let (yes, no) = if self.lang == Lang::Fr { ("Oui, la sauver", "Non") } else { ("Yes, save them", "No") };
                let pick = |i: usize, text: &str| {
                    let style = if i == *index { Style::new().fg(BG.color()).bg(GOLD.color()).add_modifier(Modifier::BOLD) } else { fg(TEXT) };
                    Span::styled(format!(" {text} "), style)
                };
                vec![Line::from(""), Line::from(vec![pick(0, yes), Span::raw("    "), pick(1, no)])]
            }
        };
        f.render_widget(Paragraph::new(lines).block(block).wrap(Wrap { trim: false }), area);
    }

    fn draw_top(&self, f: &mut Frame, area: Rect) {
        let alive = self.alive.iter().filter(|a| **a).count();
        let phase = if self.day == 0 {
            String::new()
        } else if self.night_target > 0.5 && self.winner.is_none() {
            format!("{} {}", self.ui.night, self.day)
        } else {
            format!("{} {}", self.ui.day, self.day)
        };
        let mut spans = vec![
            Span::styled(" CCWOLF ", Style::new().fg(Rgb(20.0, 10.0, 20.0).color()).bg(Rgb(232.0, 72.0, 72.0).color()).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  {phase}"), fg(TEXT).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ·  {alive} {}", self.ui.alive), fg(DIM)),
            Span::styled(format!("  ·  {}", self.lang.money(self.cost)), fg(Rgb(255.0, 210.0, 120.0))),
            Span::styled(format!("  ·  x{:.1}", self.speed), fg(DIM)),
        ];
        if self.replay {
            spans.push(Span::styled(format!("  ·  {}", self.ui.replay_tag), fg(Rgb(255.0, 200.0, 90.0)).add_modifier(Modifier::BOLD)));
        }
        let left = Line::from(spans);
        let keys = if self.sees_all() { self.ui.keys } else { self.ui.keys_alive };
        let right = Line::from(Span::styled(format!("{keys} "), fg(DIM))).alignment(Alignment::Right);
        f.render_widget(Paragraph::new(left).style(Style::new().bg(PANEL.color())), area);
        if area.width > 150 {
            f.render_widget(Paragraph::new(right), area);
        }
    }

    fn gauge(&self, p: &Panel) -> Option<Line<'static>> {
        let v = p.verdict.as_ref()?;
        let done_at = p.start + p.say.chars().count() as f32 / CPS;
        let k = ((self.clock - done_at) / 0.9).clamp(0.0, 1.0);
        if k <= 0.0 {
            return None;
        }
        let value = v.lie * (1.0 - (1.0 - k).powi(3));
        let color = canvas::ramp(&[(0.0, WITCH_GREEN), (0.5, GOLD), (1.0, WOLF_RED)], value);
        let cells = 12;
        let filled = (value * cells as f32).round() as usize;
        let pct = (value * 100.0).round() as u32;
        let label = match self.lang {
            Lang::Fr => format!(" {pct}\u{a0}% {} ", self.ui.lie),
            Lang::En => format!(" {pct}% {} ", self.ui.lie),
        };
        let mut spans = vec![
            Span::styled(format!(" {} ", self.ui.lie_detector), fg(DIM).add_modifier(Modifier::BOLD)),
            Span::styled("\u{2588}".repeat(filled), fg(color)),
            Span::styled("\u{2591}".repeat(cells - filled), fg(DIM.scale(0.6))),
            Span::styled(label, fg(color).add_modifier(Modifier::BOLD)),
        ];
        if let Some(role) = v.claim.filter(|_| k >= 1.0) {
            spans.push(Span::styled(format!("· {} {} ", self.ui.claims, self.ui.role(role)), fg(role_color(role))));
        }
        Some(Line::from(spans).alignment(Alignment::Right))
    }

    fn draw_panels(&self, f: &mut Frame, speech_area: Rect, thought_area: Rect) {
        if !self.sees_all() {
            let block = Block::bordered()
                .border_type(BorderType::Rounded)
                .border_style(fg(THOUGHT.scale(0.7)))
                .title(Span::styled(format!(" {} ", self.ui.your_role), fg(THOUGHT).add_modifier(Modifier::BOLD)))
                .style(Style::new().bg(PANEL.color()));
            f.render_widget(Paragraph::new(self.role_card()).block(block).wrap(Wrap { trim: true }), thought_area);
        }
        if let Some(input) = &self.input {
            self.draw_input(f, speech_area, input);
            if self.sees_all() {
                f.render_widget(Block::bordered().border_type(BorderType::Rounded).style(Style::new().bg(PANEL.color())), thought_area);
            }
            return;
        }
        let (title, accent) = match &self.panel {
            Some(p) => (p.title.clone(), p.accent),
            None => (self.ui.village_square.to_string(), DIM),
        };
        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(fg(accent))
            .title(Span::styled(format!(" {title} "), fg(accent).add_modifier(Modifier::BOLD)))
            .style(Style::new().bg(PANEL.color()));
        if let Some(gauge) = self.panel.as_ref().and_then(|p| self.gauge(p)) {
            block = block.title_bottom(gauge);
        }
        let thought_block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(fg(THOUGHT.scale(0.7)))
            .title(Span::styled(format!(" {} ", self.ui.secret_thought), fg(THOUGHT).add_modifier(Modifier::BOLD)))
            .style(Style::new().bg(PANEL.color()));
        let Some(p) = &self.panel else {
            f.render_widget(block, speech_area);
            if self.sees_all() {
                f.render_widget(thought_block, thought_area);
            }
            return;
        };
        let typed = ((self.clock - p.start) * CPS).max(0.0) as usize;
        let cut = |s: &str| s.chars().take(typed).collect::<String>();
        let mut lines = Vec::new();
        if let Some(w) = p.who {
            let info = &self.players[w];
            let mut head = vec![
                Span::styled(info.name.clone(), fg(Rgb::from(info.model.rgb())).add_modifier(Modifier::BOLD)),
                Span::styled(format!("  {}", self.model_label(info.model)), fg(Rgb::from(info.model.rgb()).scale(0.75))),
                Span::styled(format!("  ·  {}", info.persona), fg(DIM)),
            ];
            if self.role_visible(w) {
                head.push(Span::styled(format!("  [{}]", self.ui.role(info.role)), fg(role_color(info.role)).add_modifier(Modifier::BOLD)));
            }
            lines.push(Line::from(head));
        }
        let said = cut(&p.say);
        let cursor = if typed < p.say.chars().count() && (self.anim * 4.0) as i32 % 2 == 0 { "\u{2588}" } else { "" };
        let said = if p.who.is_some()
            && self.speaker == p.who
            && matches!(p.title.as_str(), t if t.starts_with(self.ui.village_square) || t == self.ui.wolf_council || t == self.ui.last_words)
        {
            let quoted = crate::prompts::quote(self.lang, &said);
            if typed >= p.say.chars().count() {
                quoted
            } else {
                let closing = if self.lang == Lang::Fr { 2 } else { 1 };
                quoted.chars().take(quoted.chars().count().saturating_sub(closing)).collect()
            }
        } else {
            said
        };
        let inner = block.inner(speech_area);
        let room = (inner.height as usize).saturating_sub(lines.len()).max(1);
        let mut wrapped = wrap(&format!("{said}{cursor}"), inner.width as usize);
        let skip = wrapped.len().saturating_sub(room);
        for line in wrapped.drain(skip..) {
            let (text, tail) = match line.strip_suffix(cursor).filter(|_| !cursor.is_empty()) {
                Some(text) => (text.to_string(), cursor),
                None => (line, ""),
            };
            lines.push(Line::from(vec![Span::styled(text, fg(TEXT)), Span::styled(tail, fg(accent))]));
        }
        f.render_widget(Paragraph::new(lines).block(block), speech_area);
        if !self.sees_all() {
            return;
        }

        let inner = thought_block.inner(thought_area);
        let body: Vec<Line> = if self.show_thoughts {
            let wrapped = wrap(&cut(&p.thought), inner.width as usize);
            let skip = wrapped.len().saturating_sub(inner.height as usize);
            wrapped.into_iter().skip(skip).map(|l| Line::from(Span::styled(l, fg(THOUGHT).add_modifier(Modifier::ITALIC)))).collect()
        } else {
            vec![Line::from(Span::styled("[t]", fg(DIM)))]
        };
        f.render_widget(Paragraph::new(body).block(thought_block), thought_area);
    }

    fn draw_feed(&self, f: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(fg(DIM.scale(0.6)))
            .title(Span::styled(format!(" {} ", self.ui.chronicle), fg(DIM).add_modifier(Modifier::BOLD)))
            .style(Style::new().bg(PANEL.color()));
        let inner = block.inner(area);
        let width = inner.width.saturating_sub(1) as usize;
        let rows = inner.height as usize;
        let lines: Vec<Line> = self
            .feed
            .iter()
            .rev()
            .take(rows)
            .rev()
            .map(|(text, color)| {
                let text: String = text.chars().take(width).collect();
                Line::from(Span::styled(text, fg(*color)))
            })
            .collect();
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_roster(&self, f: &mut Frame, area: Rect) {
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(fg(DIM.scale(0.6)))
            .title(Span::styled(format!(" {} ", self.ui.roster), fg(DIM).add_modifier(Modifier::BOLD)))
            .style(Style::new().bg(PANEL.color()));
        let max_votes = self.tally.iter().copied().max().unwrap_or(0).max(1);
        let lines: Vec<Line> = (0..self.players.len())
            .map(|i| {
                let p = &self.players[i];
                let alive = self.alive[i];
                let name_style = if alive { fg(TEXT).add_modifier(Modifier::BOLD) } else { fg(DIM).add_modifier(Modifier::CROSSED_OUT) };
                let mut spans = vec![
                    Span::styled("\u{25cf} ", fg(Rgb::from(p.model.rgb()))),
                    Span::styled(format!("{:<8}", p.name), name_style),
                    Span::styled(format!("{:<11}", self.model_label(p.model)), fg(Rgb::from(p.model.rgb()).scale(if alive { 0.9 } else { 0.5 }))),
                ];
                if self.role_visible(i) {
                    spans.push(Span::styled(format!("{:<11}", self.ui.role(p.role)), fg(role_color(p.role).scale(if alive { 1.0 } else { 0.6 }))));
                } else {
                    spans.push(Span::styled(format!("{:<11}", "?"), fg(DIM)));
                }
                if self.voting && self.tally[i] > 0 {
                    let bar = "\u{2588}".repeat(((self.tally[i] as f32 / max_votes as f32) * 6.0).ceil() as usize);
                    spans.push(Span::styled(format!("{bar} {}", self.tally[i]), fg(GOLD)));
                }
                Line::from(spans)
            })
            .collect();
        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn draw_recap(&self, f: &mut Frame, scene: Rect) {
        let w = 64.min(scene.width.saturating_sub(4));
        let h = (self.players.len() as u16 + 7).min(scene.height.saturating_sub(2));
        let area = Rect::new(scene.x + (scene.width - w) / 2, scene.y + scene.height.saturating_sub(h) / 2 + 2, w, h);
        f.render_widget(Clear, area);
        let mine = self.me.and_then(|me| self.players.get(me)).map(|p| p.role == Role::Wolf);
        let (title, color) = match (self.winner, &self.aborted) {
            (Some(team), _) if mine.is_some() => {
                let won = (team == Team::Wolves) == mine.unwrap_or(false);
                (format!("{} · {}", if won { self.ui.you_win } else { self.ui.you_lose }, self.ui.winner(team)), if won { GOLD } else { WOLF_RED })
            }
            (Some(team), _) => (self.ui.winner(team).to_string(), if team == Team::Wolves { WOLF_RED } else { Rgb(255.0, 214.0, 120.0) }),
            (None, Some(_)) => (self.ui.aborted.to_string(), WOLF_RED),
            (None, None) => (String::new(), TEXT),
        };
        let block = Block::bordered()
            .border_type(BorderType::Double)
            .border_style(fg(color))
            .title(Span::styled(format!(" {title} "), fg(color).add_modifier(Modifier::BOLD)))
            .title_alignment(Alignment::Center)
            .style(Style::new().bg(Rgb(8.0, 8.0, 16.0).color()));
        let mut lines = Vec::new();
        if let Some(reason) = &self.aborted {
            lines.push(Line::from(Span::styled(reason.clone(), fg(TEXT))));
        }
        for (i, p) in self.players.iter().enumerate() {
            let alive = self.alive[i];
            lines.push(Line::from(vec![
                Span::styled("\u{25cf} ", fg(Rgb::from(p.model.rgb()))),
                Span::styled(format!("{:<9}", p.name), fg(TEXT).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:<12}", self.model_label(p.model)), fg(Rgb::from(p.model.rgb()))),
                Span::styled(format!("{:<13}", self.ui.role(p.role)), fg(role_color(p.role))),
                Span::styled(if alive { self.ui.survived } else { self.ui.dead }, fg(if alive { WITCH_GREEN } else { DIM })),
            ]));
        }
        if !self.my_lies.is_empty() {
            let mean = self.my_lies.iter().sum::<f32>() / self.my_lies.len() as f32;
            let pct = (mean * 100.0).round();
            let text = if self.lang == Lang::Fr { format!("{} {pct}\u{a0}%", self.ui.your_lies) } else { format!("{} {pct}%", self.ui.your_lies) };
            lines.push(Line::from(Span::styled(text, fg(Rgb(255.0, 150.0, 60.0)))));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(vec![
            Span::styled(format!("{} : ", self.ui.total_cost), fg(DIM)),
            Span::styled(self.lang.money(self.cost), fg(Rgb(255.0, 210.0, 120.0)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("      {}", self.ui.game_over_hint), fg(DIM)),
        ]));
        f.render_widget(Paragraph::new(lines).block(block), area);
    }
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let mut word = word.to_string();
        while word.chars().count() > width {
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            let head: String = word.chars().take(width).collect();
            word = word.chars().skip(width).collect();
            lines.push(head);
        }
        let needed = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
        if needed > width && !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(&word);
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

fn crt(buf: &mut Buffer, area: Rect) {
    let (w, h) = (area.width as f32, area.height as f32);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let nx = (x - area.x) as f32 / w * 2.0 - 1.0;
            let ny = (y - area.y) as f32 / h * 2.0 - 1.0;
            let vignette = 1.0 - 0.32 * (nx * nx * 0.55 + ny * ny * 0.8).min(1.0);
            if let Some(cell) = buf.cell_mut((x, y)) {
                let half = cell.symbol() == "\u{2580}";
                if let Some(c) = Rgb::of(cell.fg) {
                    cell.fg = c.scale(vignette).color();
                }
                if let Some(c) = Rgb::of(cell.bg) {
                    let scan = if half {
                        0.9
                    } else if y % 2 == 0 {
                        0.93
                    } else {
                        1.0
                    };
                    cell.bg = c.scale(vignette * scan).color();
                }
            }
        }
    }
}

pub fn run(source: Source, opts: Options) -> io::Result<()> {
    let mut app = App::new(source, opts);
    let mut terminal = ratatui::try_init()?;
    let truecolor = canvas::truecolor();
    let mut last = Instant::now();
    let result = loop {
        let now = Instant::now();
        let dt = (now - last).as_secs_f32().min(0.2);
        last = now;
        app.tick(dt);
        let drawn = terminal.draw(|f| {
            app.draw(f);
            if !truecolor {
                canvas::to_256_colors(f.buffer_mut());
            }
        });
        if let Err(e) = drawn {
            break Err(e);
        }
        match event::poll(Duration::from_millis(30)) {
            Ok(true) => {
                if let Ok(TermEvent::Key(k)) = event::read()
                    && k.kind == KeyEventKind::Press
                {
                    if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
                        app.quit = true;
                    } else {
                        app.key(k.code);
                    }
                }
            }
            Ok(false) => {}
            Err(e) => break Err(e),
        }
        if app.quit {
            break Ok(());
        }
    };
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Agent, Engine, SEATS, ScriptedAgent, deal};
    use crate::model::lineup;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    struct RoleJudge;

    impl crate::game::Judge for RoleJudge {
        fn judge(&self, t: &crate::game::Testimony) -> Option<Verdict> {
            let wolf = t.role == Role::Wolf;
            let accuses = t.names.iter().position(|n| *n != t.speaker);
            Some(Verdict { lie: if wolf { 0.87 } else { 0.12 }, convincing: Some(3.4), claim: wolf.then_some(Role::Villager), accuses })
        }
    }

    fn recorded(lang: Lang) -> Vec<Event> {
        let players = deal(4, lang, &lineup("mixed").unwrap());
        let agents: Vec<Box<dyn Agent>> = (0..SEATS).map(|i| Box::new(ScriptedAgent::new(4, i)) as Box<dyn Agent>).collect();
        let mut events = Vec::new();
        let judge: std::sync::Arc<dyn crate::game::Judge> = std::sync::Arc::new(RoleJudge);
        Engine::new(4, lang, players, agents).judged_by(Some(judge)).run(&mut |e: &Event| events.push(e.clone()));
        events
    }

    #[test]
    fn a_whole_game_plays_through_the_screen_without_panicking() {
        for lang in [Lang::Fr, Lang::En] {
            let opts = Options { lang, crt: true, speed: 1.0, lineup: lineup("mixed").unwrap(), replay: true, jev: false, human: None };
            let mut app = App::new(Source::Replay(recorded(lang)), opts);
            let mut terminal = Terminal::new(TestBackend::new(140, 44)).unwrap();
            terminal.draw(|f| app.draw(f)).unwrap();
            app.start();
            for _ in 0..4000 {
                app.tick(0.1);
                app.skip();
                terminal.draw(|f| app.draw(f)).unwrap();
                if app.winner.is_some() {
                    break;
                }
            }
            assert!(app.winner.is_some(), "the replay should reach the end");
            let mut small = Terminal::new(TestBackend::new(60, 20)).unwrap();
            small.draw(|f| app.draw(f)).unwrap();
        }
    }

    fn snapshot(buf: &Buffer, path: &std::path::Path) {
        let (cw, ch) = (8usize, 16usize);
        let (w, h) = (buf.area.width as usize * cw, buf.area.height as usize * ch);
        let mut px = vec![0u8; w * h * 3];
        let mut put = |x: usize, y: usize, c: Rgb| {
            if x < w && y < h {
                let i = (y * w + x) * 3;
                px[i] = c.0.clamp(0.0, 255.0) as u8;
                px[i + 1] = c.1.clamp(0.0, 255.0) as u8;
                px[i + 2] = c.2.clamp(0.0, 255.0) as u8;
            }
        };
        for row in 0..buf.area.height {
            for col in 0..buf.area.width {
                let cell = buf.cell((col, row)).unwrap();
                let fgc = Rgb::of(cell.fg).unwrap_or(TEXT);
                let bgc = Rgb::of(cell.bg).unwrap_or(BG);
                let (x0, y0) = (col as usize * cw, row as usize * ch);
                for dy in 0..ch {
                    for dx in 0..cw {
                        let c = if cell.symbol() == "\u{2580}" { if dy < ch / 2 { fgc } else { bgc } } else { bgc };
                        put(x0 + dx, y0 + dy, c);
                    }
                }
                let sym = cell.symbol().chars().next().unwrap_or(' ');
                if sym == '\u{2580}' || sym == ' ' {
                    continue;
                }
                if let Some(rows) = canvas::glyph(sym) {
                    for (gy, line) in rows.iter().enumerate() {
                        for (gx, bit) in line.chars().enumerate() {
                            if bit == '#' {
                                put(x0 + 1 + gx, y0 + 4 + gy, fgc);
                            }
                        }
                    }
                } else if sym == '\u{2588}' {
                    for dy in 0..ch {
                        for dx in 0..cw {
                            put(x0 + dx, y0 + dy, fgc);
                        }
                    }
                } else if !sym.is_whitespace() {
                    for dy in 7..10 {
                        for dx in 2..6 {
                            put(x0 + dx, y0 + dy, fgc);
                        }
                    }
                }
            }
        }
        let mut file = format!("P6\n{w} {h}\n255\n").into_bytes();
        file.extend(px);
        std::fs::write(path, file).unwrap();
    }

    #[test]
    fn snapshots_for_eyeballing() {
        let Ok(dir) = std::env::var("CCWOLF_SNAPSHOTS") else { return };
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        let opts = Options { lang: Lang::Fr, crt: true, speed: 1.0, lineup: lineup("mixed").unwrap(), replay: false, jev: true, human: None };
        let events = match std::env::var("CCWOLF_SNAPSHOT_REPLAY") {
            Ok(path) => crate::record::load(std::path::Path::new(&path)).unwrap(),
            Err(_) => recorded(Lang::Fr),
        };
        let mut app = App::new(Source::Replay(events), opts);
        let mut terminal = Terminal::new(TestBackend::new(170, 52)).unwrap();
        app.anim = 3.0;
        terminal.draw(|f| app.draw(f)).unwrap();
        snapshot(terminal.backend().buffer(), &dir.join("00-title.ppm"));
        app.start();
        let marks = [2.0, 5.0, 9.0, 14.0, 19.0, 26.0, 40.0, 44.0, 47.0, 60.0, 90.0, 140.0];
        let mut next = 0;
        let mut t = 0.0;
        while next < marks.len() && t < 400.0 {
            app.tick(0.05);
            t += 0.05;
            if t >= marks[next] {
                terminal.draw(|f| app.draw(f)).unwrap();
                snapshot(terminal.backend().buffer(), &dir.join(format!("{:02}-t{}.ppm", next + 1, marks[next] as u32)));
                next += 1;
            }
        }
    }

    #[test]
    fn wrapping_keeps_every_word_within_the_width() {
        let text = "Pauvre Marie, c'est affreux. Je suis Léon, l'ancien maire, et je vous prie de croire que je n'ai rien à cacher. Anticonstitutionnellement!";
        for width in [8, 12, 30, 60] {
            let lines = wrap(text, width);
            assert!(lines.iter().all(|l| l.chars().count() <= width), "{width}: {lines:?}");
            assert_eq!(lines.join(" ").split_whitespace().collect::<String>(), text.split_whitespace().collect::<String>());
        }
    }

    #[test]
    fn a_human_plays_a_whole_game_from_the_keyboard_and_only_sees_their_share() {
        let seed = 8;
        let mut players = deal(seed, Lang::Fr, &lineup("mixed").unwrap());
        let me = players.iter().position(|p| p.role == Role::Villager).unwrap();
        players[me].model = Model::Human;
        let wolves: Vec<String> = players.iter().filter(|p| p.role == Role::Wolf).map(|p| p.name.clone()).collect();
        let mut agents: Vec<Box<dyn Agent>> = (0..SEATS).map(|i| Box::new(ScriptedAgent::new(seed, i)) as Box<dyn Agent>).collect();
        let (agent, link) = human::pair(me);
        agents[me] = Box::new(agent);
        let table = players.clone();
        let launch: Launch = Box::new(move |tx| {
            Engine::new(seed, Lang::Fr, table, agents).run(&mut |e: &Event| {
                let _ = tx.send(e.clone());
            });
        });
        let opts = Options { lang: Lang::Fr, crt: false, speed: 1.0, lineup: lineup("mixed").unwrap(), replay: false, jev: false, human: Some(link) };
        let mut app = App::new(Source::Live(launch), opts);
        let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
        let shots = std::env::var("CCWOLF_SNAPSHOTS").ok().map(std::path::PathBuf::from);
        app.key(KeyCode::Enter);
        assert!(app.intro);
        for _ in 0..5 {
            app.tick(0.05);
        }
        terminal.draw(|f| app.draw(f)).unwrap();
        if let Some(dir) = &shots {
            std::fs::create_dir_all(dir).unwrap();
            snapshot(terminal.backend().buffer(), &dir.join("p0-intro.ppm"));
        }
        app.key(KeyCode::Esc);
        assert!(app.confirm_quit && !app.quit, "Esc in the intro should ask before leaving a paid game");
        app.key(KeyCode::Esc);
        assert!(!app.confirm_quit && !app.quit && app.intro);
        app.key(KeyCode::Enter);
        let mut answered = 0;
        let mut asked_to_leave = false;
        for _ in 0..60000 {
            app.tick(0.05);
            app.skip();
            if let Some(input) = app.input.clone() {
                if let (Some(dir), true) = (&shots, answered < 4) {
                    app.key(KeyCode::Right);
                    if matches!(input, Input::Speech { .. }) {
                        for c in "Moi, je n'ai rien vu".chars() {
                            app.key(KeyCode::Char(c));
                        }
                    }
                    terminal.draw(|f| app.draw(f)).unwrap();
                    snapshot(terminal.backend().buffer(), &dir.join(format!("p{}-input.ppm", answered + 1)));
                    if let Some(Input::Speech { buffer, .. }) = &mut app.input {
                        buffer.clear();
                    }
                }
                if matches!(input, Input::Speech { .. }) {
                    for c in "je suis innocent".chars() {
                        app.key(KeyCode::Char(c));
                    }
                    if !asked_to_leave {
                        asked_to_leave = true;
                        app.key(KeyCode::Esc);
                        assert!(app.confirm_quit && !app.quit);
                        terminal.draw(|f| app.draw(f)).unwrap();
                        let screen: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
                        assert!(screen.contains("QUITTER LA PARTIE"));
                        if let Some(dir) = &shots {
                            snapshot(terminal.backend().buffer(), &dir.join("p9-quit.ppm"));
                        }
                        app.key(KeyCode::Char('n'));
                        assert!(!app.quit);
                        assert!(
                            matches!(&app.input, Some(Input::Speech { buffer, .. }) if buffer == "je suis innocent"),
                            "going back keeps the line being typed"
                        );
                    }
                }
                app.key(KeyCode::Right);
                app.key(KeyCode::Enter);
                answered += 1;
            }
            terminal.draw(|f| app.draw(f)).unwrap();
            if !app.sees_all() && !app.players.is_empty() {
                let screen: String = terminal.backend().buffer().content().iter().map(|c| c.symbol()).collect();
                assert!(!screen.contains("veut dévorer") && !screen.contains("CONCILIABULE"), "a villager saw the wolves' night");
                assert!(!screen.contains("[t] pensées"), "the asides key does nothing while the player is alive");
                for w in &wolves {
                    let i = app.players.iter().position(|p| &p.name == w).unwrap();
                    assert!(!app.role_visible(i) || !app.alive[i], "a living wolf was unmasked");
                }
            }
            if app.winner.is_some() {
                break;
            }
        }
        assert!(app.winner.is_some(), "the game should end");
        assert!(answered > 0, "the player should have been asked to act");
        assert!(app.sees_all());
        assert!(asked_to_leave, "the player should have typed at least one line");
        app.key(KeyCode::Char('q'));
        assert!(app.quit, "once the game is over, q leaves at once");
    }
}
