use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::game::Event;

pub struct Recorder {
    path: PathBuf,
    out: BufWriter<File>,
}

impl Recorder {
    pub fn create(dir: &Path, seed: u64) -> std::io::Result<Recorder> {
        std::fs::create_dir_all(dir)?;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let path = dir.join(format!("{stamp}-{seed}.jsonl"));
        let out = BufWriter::new(File::create(&path)?);
        Ok(Recorder { path, out })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&mut self, event: &Event) {
        if event.is_recorded()
            && let Ok(line) = serde_json::to_string(event)
        {
            let _ = writeln!(self.out, "{line}");
            let _ = self.out.flush();
        }
    }
}

pub fn default_dir() -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".ccwolf").join("games")
}

pub fn parse(text: &str) -> Result<Vec<Event>, String> {
    text.lines().filter(|l| !l.trim().is_empty()).enumerate().map(|(i, l)| serde_json::from_str(l).map_err(|e| format!("line {}: {e}", i + 1))).collect()
}

pub fn load(path: &Path) -> Result<Vec<Event>, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut text = String::new();
    for line in BufReader::new(file).lines() {
        text.push_str(&line.map_err(|e| e.to_string())?);
        text.push('\n');
    }
    let events = parse(&text)?;
    match events.first() {
        Some(Event::Setup { .. }) => Ok(events),
        _ => Err(format!("{}: not a ccwolf game", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Agent, Engine, SEATS, ScriptedAgent, deal};
    use crate::lang::Lang;
    use crate::model::lineup;

    #[test]
    fn a_recorded_game_replays_identically() {
        let dir = std::env::temp_dir().join(format!("ccwolf-test-{}", std::process::id()));
        let players = deal(21, Lang::En, &lineup("mixed").unwrap());
        let agents: Vec<Box<dyn Agent>> = (0..SEATS).map(|i| Box::new(ScriptedAgent::new(21, i)) as Box<dyn Agent>).collect();
        let mut recorder = Recorder::create(&dir, 21).unwrap();
        let mut live = Vec::new();
        Engine::new(21, Lang::En, players, agents).run(&mut |e: &Event| {
            recorder.write(e);
            if e.is_recorded() {
                live.push(e.clone());
            }
        });
        let replayed = load(recorder.path()).unwrap();
        assert_eq!(replayed, live);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
