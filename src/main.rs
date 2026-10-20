mod claude;
mod game;
mod human;
mod jev;
mod lang;
mod model;
mod prompts;
mod record;
mod rng;
mod stats;
mod ui;

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Sender;

use claude::{Claude, LlmAgent};
use game::{Agent, Engine, Event, Judge, PlayerInfo, ScriptedAgent, deal};
use lang::Lang;
use model::Model;
use record::Recorder;

const DEMO_FR: &str = include_str!("../replays/demo-fr.jsonl");
const DEMO_EN: &str = include_str!("../replays/demo-en.jsonl");

const HELP: &str = "ccwolf: werewolf in the terminal, played by Claude models

USAGE
  ccwolf                     watch 8 Claude models play (needs ANTHROPIC_API_KEY;
                             with TYPESAFE_API_KEY, Jev scores every speech as a lie detector)
  ccwolf --play              take a seat yourself: you get a character and a secret role,
                             and try to win against seven Claude models
  ccwolf --demo              watch a recorded game, no API key needed
  ccwolf --replay FILE       watch one of your recorded games again
  ccwolf stats [DIR]         who lies best: results by model over your recorded games
  ccwolf jev-check           try the Jev lie detector on three sample speeches
  ccwolf judge FILE...       add Jev verdicts to recorded games (--force to judge again)

OPTIONS
  --lang fr|en               language of the game and of the players (default: your locale)
  --lineup SPEC              mixed (default: 2 x Haiku, Sonnet, Opus, Fable), haiku, sonnet,
                             opus, fable, or 8 comma-separated models
  --seed N                   deal the same roles and seats again
  --speed X                  playback speed, 0.4 to 6 (default 1)
  --no-crt                   turn off the scanlines and the vignette
  --headless                 no screen: print the game as text
  --games N                  with --headless, play N games in a row
  --out DIR                  where games are recorded (default ~/.ccwolf/games, none with --scripted)
  --scripted                 players answer at random without the API, to test the screen
  --effort low|medium|high   how hard the models think before answering (default: each
                             model's own default; low is cheaper and faster)
  --no-jev                   do not call the Jev lie detector even if TYPESAFE_API_KEY is set
  -h, --help                 this help
  -V, --version              the version

KEYS
  t thoughts   r roles   c crt   space pause   right skip   + - speed   q quit";

struct Args {
    lang: Lang,
    lineup: Vec<Model>,
    seed: Option<u64>,
    speed: f32,
    crt: bool,
    demo: bool,
    replay: Option<PathBuf>,
    headless: bool,
    games: u32,
    scripted: bool,
    out: Option<PathBuf>,
    stats: Option<PathBuf>,
    jev: bool,
    jev_check: bool,
    effort: Option<String>,
    judge: Vec<PathBuf>,
    force: bool,
    video: Option<PathBuf>,
    play: bool,
    promo: Option<PathBuf>,
}

fn parse_args() -> Result<Option<Args>, String> {
    let mut args = Args {
        lang: Lang::from_env(),
        lineup: model::lineup("mixed").expect("mixed lineup"),
        seed: None,
        speed: 1.0,
        crt: true,
        demo: false,
        replay: None,
        headless: false,
        games: 1,
        scripted: false,
        out: None,
        stats: None,
        jev: true,
        jev_check: false,
        effort: None,
        judge: Vec::new(),
        force: false,
        video: None,
        play: false,
        promo: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or(format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" => {
                println!("{HELP}");
                return Ok(None);
            }
            "-V" | "--version" => {
                println!("ccwolf {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--lang" => args.lang = Lang::parse(&value("--lang")?).ok_or("--lang takes fr or en")?,
            "--lineup" => args.lineup = model::lineup(&value("--lineup")?).ok_or("--lineup takes mixed, one model name, or 8 comma-separated models")?,
            "--seed" => args.seed = Some(value("--seed")?.parse().map_err(|_| "--seed takes a number")?),
            "--speed" => args.speed = value("--speed")?.parse::<f32>().map_err(|_| "--speed takes a number")?.clamp(0.4, 6.0),
            "--games" => args.games = value("--games")?.parse().map_err(|_| "--games takes a number")?,
            "--out" => args.out = Some(PathBuf::from(value("--out")?)),
            "--replay" => args.replay = Some(PathBuf::from(value("--replay")?)),
            "--no-crt" => args.crt = false,
            "--video" => args.video = Some(PathBuf::from(value("--video")?)),
            "--promo" => args.promo = Some(PathBuf::from(value("--promo")?)),
            "--demo" => args.demo = true,
            "--play" => args.play = true,
            "--headless" => args.headless = true,
            "--scripted" => args.scripted = true,
            "--no-jev" => args.jev = false,
            "--effort" => {
                let effort = value("--effort")?;
                if !["low", "medium", "high"].contains(&effort.as_str()) {
                    return Err("--effort takes low, medium or high".into());
                }
                args.effort = Some(effort);
            }
            "jev-check" => args.jev_check = true,
            "judge" => {
                args.judge = it.by_ref().map(PathBuf::from).collect();
                args.force = args.judge.iter().any(|p| p.as_os_str() == "--force");
                args.judge.retain(|p| p.as_os_str() != "--force");
                if args.judge.is_empty() {
                    return Err("judge needs at least one recorded game".into());
                }
            }
            "stats" => args.stats = Some(it.next().map(PathBuf::from).unwrap_or_else(record::default_dir)),
            other => return Err(format!("unknown argument {other}, see ccwolf --help")),
        }
    }
    Ok(Some(args))
}

fn agents(args: &Args, seed: u64, players: &[PlayerInfo]) -> Result<Vec<Box<dyn Agent>>, String> {
    if args.scripted {
        return Ok((0..players.len()).map(|i| Box::new(ScriptedAgent::new(seed, i)) as Box<dyn Agent>).collect());
    }
    let claude = Claude::from_env().ok_or("ANTHROPIC_API_KEY is not set. Set it to play, or run ccwolf --demo to watch a recorded game.")?;
    Ok((0..players.len()).map(|i| Box::new(LlmAgent::new(claude.clone(), i, args.lang, players, args.effort.clone())) as Box<dyn Agent>).collect())
}

fn judge(args: &Args) -> Option<Arc<dyn Judge>> {
    if !args.jev {
        return None;
    }
    jev::Jev::from_env().map(|j| Arc::new(j) as Arc<dyn Judge>)
}

fn jev_check() -> Result<(), String> {
    let jev = jev::Jev::from_env().ok_or("TYPESAFE_API_KEY is not set")?;
    for (t, expected) in jev::samples() {
        let started = std::time::Instant::now();
        let v = jev.ask(&t)?;
        let accused = v.accuses.map(|i| t.names[i].clone()).unwrap_or_else(|| "nobody".into());
        let claim = v.claim.map(|r| format!("{r:?}").to_lowercase()).unwrap_or_else(|| "none".into());
        let convincing = v.convincing.map(|c| format!("{c:.1}/5")).unwrap_or_else(|| "-".into());
        println!("{} ({expected})", t.speaker);
        println!("  lie {:.0}%, convincing {convincing}, claims {claim}, accuses {accused}, {} ms", v.lie * 100.0, started.elapsed().as_millis());
    }
    Ok(())
}

fn judge_files(args: &Args) -> Result<(), String> {
    let jev = jev::Jev::from_env().ok_or("TYPESAFE_API_KEY is not set")?;
    for path in &args.judge {
        let mut events = record::load(path)?;
        let (judged, failed) = game::judge_recording(&mut events, &jev, args.force);
        if judged > 0 {
            record::save(path, &events)?;
        }
        println!("{}: {judged} speeches judged, {failed} failed", path.display());
    }
    Ok(())
}

fn recorder(args: &Args, seed: u64) -> Result<Option<Recorder>, String> {
    let dir = match (&args.out, args.scripted) {
        (Some(dir), _) => dir.clone(),
        (None, true) => return Ok(None),
        (None, false) => record::default_dir(),
    };
    Recorder::create(&dir, seed).map(Some).map_err(|e| e.to_string())
}

fn headless(args: &Args) -> Result<(), String> {
    let base = args.seed.unwrap_or_else(rng::seed_from_clock);
    for g in 0..args.games.max(1) {
        let seed = base + g as u64;
        let players = deal(seed, args.lang, &args.lineup);
        let agents = agents(args, seed, &players)?;
        let mut recorder = recorder(args, seed)?;
        let mut cost = 0.0;
        let mut out = std::io::stdout().lock();
        Engine::new(seed, args.lang, players.clone(), agents).judged_by(judge(args)).run(&mut |e: &Event| {
            if let Some(r) = recorder.as_mut() {
                r.write(e);
            }
            if let Event::Usage { cost: c, .. } = e {
                cost += c;
            }
            for line in prompts::narrate(args.lang, e, &players) {
                if writeln!(out, "{line}").is_err() {
                    break;
                }
            }
        });
        let saved = recorder.as_ref().map(|r| r.path().display().to_string()).unwrap_or_default();
        let colon = if args.lang == Lang::Fr { " :" } else { ":" };
        let summary = format!("{}{colon} {}\n{saved}\n", args.lang.ui().total_cost, args.lang.money(cost));
        if writeln!(out, "{summary}").is_err() {
            return Ok(());
        }
    }
    Ok(())
}

#[cfg(feature = "video")]
fn video(events: Vec<Event>, options: ui::Options, out: &std::path::Path, promo: Option<&std::path::Path>) -> Result<(), String> {
    let video = ui::video::VideoOptions {
        out: out.to_path_buf(),
        cols: 192,
        rows: 54,
        fps: 30,
        font: PathBuf::from("/System/Library/Fonts/SFNSMono.ttf"),
        italic_font: PathBuf::from("/System/Library/Fonts/SFNSMonoItalic.ttf"),
        cell_w: 10,
        cell_h: 20,
        title_seconds: 3.0,
        tail_seconds: 6.0,
    };
    match promo {
        Some(board) => ui::promo::render(events, options, video, board),
        None => ui::video::render(events, options, video),
    }
}

#[cfg(not(feature = "video"))]
fn video(_: Vec<Event>, _: ui::Options, _: &std::path::Path, _: Option<&std::path::Path>) -> Result<(), String> {
    Err("this build has no video support: cargo build --release --features video".into())
}

fn screen_error(e: std::io::Error) -> String {
    format!("cannot open the terminal screen ({e}), run ccwolf in an interactive terminal or use --headless")
}

fn watch(args: Args) -> Result<(), String> {
    let judge = judge(&args);
    let jev = judge.is_some();
    let options = |lang: Lang, replay: bool| ui::Options { lang, crt: args.crt, speed: args.speed, lineup: args.lineup.clone(), replay, jev, human: None };
    if args.demo || args.replay.is_some() {
        let events = match &args.replay {
            Some(path) => record::load(path)?,
            None => record::parse(if args.lang == Lang::Fr { DEMO_FR } else { DEMO_EN })?,
        };
        if !matches!(events.first(), Some(Event::Setup { .. })) {
            return Err("this build has no recorded demo game".into());
        }
        let lang = match events.first() {
            Some(Event::Setup { lang, .. }) => *lang,
            _ => args.lang,
        };
        if let Some(out) = &args.video {
            return video(events, options(lang, true), out, args.promo.as_deref());
        }
        return ui::run(ui::Source::Replay(events), options(lang, true)).map_err(screen_error);
    }
    let seed = args.seed.unwrap_or_else(rng::seed_from_clock);
    let mut players = deal(seed, args.lang, &args.lineup);
    let me = args.play.then(|| rng::Rng::new(seed ^ 0xBEEF).below(players.len()));
    if let Some(me) = me {
        players[me].model = Model::Human;
    }
    let mut agents = agents(&args, seed, &players)?;
    let mut seat = None;
    if let Some(me) = me {
        let (agent, link) = human::pair(me);
        agents[me] = Box::new(agent);
        seat = Some(link);
    }
    let mut recorder = recorder(&args, seed)?;
    let path = recorder.as_ref().map(|r| r.path().to_path_buf());
    let lang = args.lang;
    let launch: ui::Launch = Box::new(move |tx: Sender<Event>| {
        Engine::new(seed, lang, players, agents).judged_by(judge).run(&mut |e: &Event| {
            if let Some(r) = recorder.as_mut() {
                r.write(e);
            }
            let _ = tx.send(e.clone());
        });
    });
    let mut opts = options(lang, false);
    opts.human = seat;
    ui::run(ui::Source::Live(launch), opts).map_err(screen_error)?;
    if let Some(path) = path {
        println!("ccwolf --replay {}", path.display());
    }
    Ok(())
}

fn main() {
    let result = match parse_args() {
        Ok(None) => Ok(()),
        Ok(Some(args)) if args.jev_check => jev_check(),
        Ok(Some(args)) if !args.judge.is_empty() => judge_files(&args),
        Ok(Some(args)) if args.stats.is_some() => stats::print(args.stats.as_deref().expect("stats dir"), args.lang),
        Ok(Some(args)) if args.headless => headless(&args),
        Ok(Some(args)) => watch(args),
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        eprintln!("ccwolf: {e}");
        std::process::exit(1);
    }
}
