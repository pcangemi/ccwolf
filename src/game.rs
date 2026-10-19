use serde::{Deserialize, Serialize};

use crate::lang::Lang;
use crate::model::{Model, Usage};
use crate::rng::Rng;

pub const SEATS: usize = 8;
const MAX_DAYS: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Wolf,
    Seer,
    Witch,
    Villager,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Team {
    Wolves,
    Village,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerInfo {
    pub name: String,
    pub persona: String,
    pub model: Model,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Event {
    Setup { version: String, seed: u64, lang: Lang, players: Vec<PlayerInfo> },
    Night { day: u32 },
    WolfTalk { player: usize, thought: String, say: String },
    WolfPick { player: usize, target: usize, thought: String },
    Pack { target: usize },
    SeerLook { player: usize, target: usize, wolf: bool, thought: String },
    Witch { player: usize, victim: Option<usize>, save: bool, poison: Option<usize>, thought: String },
    Dawn { day: u32, deaths: Vec<usize> },
    Speech { player: usize, round: u32, thought: String, say: String },
    Vote { player: usize, target: usize, thought: String },
    Verdict { eliminated: Option<usize>, tally: Vec<u32> },
    LastWords { player: usize, thought: String, say: String },
    Thinking { player: usize },
    Usage { player: usize, usage: Usage, cost: f64 },
    GameOver { winner: Team },
    Aborted { reason: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    All,
    Wolves,
    Only(usize),
    Nobody,
}

impl Event {
    pub fn audience(&self) -> Audience {
        match self {
            Event::Night { .. }
            | Event::Dawn { .. }
            | Event::Speech { .. }
            | Event::Vote { .. }
            | Event::Verdict { .. }
            | Event::LastWords { .. }
            | Event::GameOver { .. } => Audience::All,
            Event::WolfTalk { .. } | Event::WolfPick { .. } | Event::Pack { .. } => Audience::Wolves,
            Event::SeerLook { player, .. } | Event::Witch { player, .. } => Audience::Only(*player),
            Event::Setup { .. } | Event::Thinking { .. } | Event::Usage { .. } | Event::Aborted { .. } => Audience::Nobody,
        }
    }

    pub fn visible_to(&self, viewer: usize, players: &[PlayerInfo]) -> bool {
        match self.audience() {
            Audience::All => true,
            Audience::Wolves => players[viewer].role == Role::Wolf,
            Audience::Only(p) => p == viewer,
            Audience::Nobody => false,
        }
    }

    pub fn is_recorded(&self) -> bool {
        !matches!(self, Event::Thinking { .. })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Task {
    WolfTalk { day: u32, targets: Vec<usize> },
    WolfPick { day: u32, targets: Vec<usize> },
    Seer { day: u32, targets: Vec<usize> },
    Witch { day: u32, victim: Option<usize>, can_save: bool, can_poison: bool, targets: Vec<usize> },
    Speak { day: u32, round: u32 },
    Vote { day: u32, targets: Vec<usize> },
    LastWords,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Speak { thought: String, say: String },
    Choose { thought: String, target: Option<usize> },
    Potion { thought: String, save: bool, poison: Option<usize> },
}

#[derive(Clone, Debug, Default)]
pub struct Reply {
    pub action: Option<Action>,
    pub usage: Usage,
}

pub trait Agent: Send {
    fn act(&mut self, news: &[String], task: &Task) -> Result<Reply, String>;
}

pub fn deal(seed: u64, lang: Lang, lineup: &[Model]) -> Vec<PlayerInfo> {
    let ui = lang.ui();
    let mut rng = Rng::new(seed);
    let mut roles = vec![Role::Wolf, Role::Wolf, Role::Seer, Role::Witch, Role::Villager, Role::Villager, Role::Villager, Role::Villager];
    rng.shuffle(&mut roles);
    let mut characters: Vec<usize> = (0..ui.characters.len()).collect();
    rng.shuffle(&mut characters);
    let mut models = lineup.to_vec();
    rng.shuffle(&mut models);
    (0..SEATS)
        .map(|i| PlayerInfo {
            name: ui.characters[characters[i]].0.to_string(),
            persona: ui.characters[characters[i]].1.to_string(),
            model: models[i % models.len()],
            role: roles[i],
        })
        .collect()
}

pub struct Engine {
    pub lang: Lang,
    pub seed: u64,
    pub players: Vec<PlayerInfo>,
    agents: Vec<Box<dyn Agent>>,
    alive: Vec<bool>,
    log: Vec<Event>,
    seen: Vec<usize>,
    heal_left: bool,
    poison_left: bool,
    rng: Rng,
}

type Emit<'a> = &'a mut dyn FnMut(&Event);

impl Engine {
    pub fn new(seed: u64, lang: Lang, players: Vec<PlayerInfo>, agents: Vec<Box<dyn Agent>>) -> Self {
        let n = players.len();
        Self {
            lang,
            seed,
            players,
            agents,
            alive: vec![true; n],
            log: Vec::new(),
            seen: vec![0; n],
            heal_left: true,
            poison_left: true,
            rng: Rng::new(seed ^ 0xC0FFEE),
        }
    }

    pub fn run(mut self, emit: Emit) -> Option<Team> {
        let setup = Event::Setup { version: env!("CARGO_PKG_VERSION").to_string(), seed: self.seed, lang: self.lang, players: self.players.clone() };
        self.push(setup, emit);
        for day in 1..=MAX_DAYS {
            let outcome = self.night(day, emit).and_then(|_| match self.winner() {
                Some(team) => Ok(Some(team)),
                None => self.day(day, emit).map(|_| self.winner()),
            });
            match outcome {
                Ok(Some(team)) => {
                    self.push(Event::GameOver { winner: team }, emit);
                    return Some(team);
                }
                Ok(None) => {}
                Err(reason) => {
                    self.push(Event::Aborted { reason }, emit);
                    return None;
                }
            }
        }
        let team = if self.living(Role::Wolf).is_empty() { Team::Village } else { Team::Wolves };
        self.push(Event::GameOver { winner: team }, emit);
        Some(team)
    }

    fn push(&mut self, event: Event, emit: Emit) {
        emit(&event);
        self.log.push(event);
    }

    fn living(&self, role: Role) -> Vec<usize> {
        (0..self.players.len()).filter(|&p| self.alive[p] && self.players[p].role == role).collect()
    }

    fn alive_list(&self) -> Vec<usize> {
        (0..self.players.len()).filter(|&p| self.alive[p]).collect()
    }

    pub fn winner(&self) -> Option<Team> {
        let wolves = self.living(Role::Wolf).len();
        let others = self.alive_list().len() - wolves;
        if wolves == 0 {
            Some(Team::Village)
        } else if wolves >= others {
            Some(Team::Wolves)
        } else {
            None
        }
    }

    fn news_for(&mut self, p: usize) -> Vec<String> {
        let start = self.seen[p];
        self.seen[p] = self.log.len();
        self.log[start..].iter().filter(|e| e.visible_to(p, &self.players)).filter_map(|e| crate::prompts::describe(self.lang, e, p, &self.players)).collect()
    }

    fn ask(&mut self, p: usize, task: Task, emit: Emit) -> Result<Action, String> {
        let news = self.news_for(p);
        let reply = self.agents[p].act(&news, &task)?;
        self.record_usage(p, &reply.usage, emit);
        Ok(reply.action.unwrap_or_else(|| fallback(&task)))
    }

    fn ask_many(&mut self, requests: Vec<(usize, Task)>, emit: Emit) -> Result<Vec<(usize, Action)>, String> {
        let mut batch: Vec<(usize, Vec<String>, Task)> = Vec::new();
        for (p, task) in requests {
            let news = self.news_for(p);
            batch.push((p, news, task));
        }
        let replies: Vec<(usize, Result<Reply, String>, Task)> = std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .agents
                .iter_mut()
                .enumerate()
                .filter_map(|(i, agent)| {
                    let position = batch.iter().position(|(p, _, _)| *p == i)?;
                    let (p, news, task) = batch[position].clone();
                    Some(scope.spawn(move || {
                        let result = agent.act(&news, &task);
                        (p, result, task)
                    }))
                })
                .collect();
            handles.into_iter().map(|h| h.join().expect("agent thread panicked")).collect()
        });
        let mut actions = Vec::new();
        for (p, result, task) in replies {
            let reply = result?;
            self.record_usage(p, &reply.usage, emit);
            actions.push((p, reply.action.unwrap_or_else(|| fallback(&task))));
        }
        Ok(actions)
    }

    fn record_usage(&mut self, p: usize, usage: &Usage, emit: Emit) {
        if *usage != Usage::default() {
            let cost = self.players[p].model.cost(usage);
            self.push(Event::Usage { player: p, usage: *usage, cost }, emit);
        }
    }

    fn valid_target(&mut self, wanted: Option<usize>, targets: &[usize]) -> usize {
        match wanted {
            Some(t) if targets.contains(&t) => t,
            _ => self.rng.pick(targets).unwrap_or(0),
        }
    }

    fn night(&mut self, day: u32, emit: Emit) -> Result<(), String> {
        self.push(Event::Night { day }, emit);
        let wolves = self.living(Role::Wolf);
        let prey: Vec<usize> = self.alive_list().into_iter().filter(|p| self.players[*p].role != Role::Wolf).collect();

        for &w in wolves.iter().filter(|_| wolves.len() > 1) {
            self.push(Event::Thinking { player: w }, emit);
            let (thought, say) = speech_of(self.ask(w, Task::WolfTalk { day, targets: prey.clone() }, emit)?);
            self.push(Event::WolfTalk { player: w, thought, say }, emit);
        }

        let picks = self.ask_many(wolves.iter().map(|&w| (w, Task::WolfPick { day, targets: prey.clone() })).collect(), emit)?;
        let mut chosen = Vec::new();
        for (w, action) in picks {
            let (thought, wanted) = choice_of(action);
            let target = self.valid_target(wanted, &prey);
            chosen.push(target);
            self.push(Event::WolfPick { player: w, target, thought }, emit);
        }
        let victim = majority(&chosen);
        if let Some(target) = victim {
            self.push(Event::Pack { target }, emit);
        }

        let mut requests = Vec::new();
        if let Some(&seer) = self.living(Role::Seer).first() {
            let targets: Vec<usize> = self.alive_list().into_iter().filter(|&p| p != seer).collect();
            requests.push((seer, Task::Seer { day, targets }));
        }
        if let Some(&witch) = self.living(Role::Witch).first() {
            let targets: Vec<usize> = self.alive_list().into_iter().filter(|&p| p != witch).collect();
            requests.push((witch, Task::Witch { day, victim, can_save: self.heal_left && victim.is_some(), can_poison: self.poison_left, targets }));
        }
        let mut saved = false;
        let mut poisoned = None;
        for (p, action) in self.ask_many(requests, emit)? {
            if self.players[p].role == Role::Seer {
                let targets: Vec<usize> = self.alive_list().into_iter().filter(|&q| q != p).collect();
                let (thought, wanted) = choice_of(action);
                let target = self.valid_target(wanted, &targets);
                let wolf = self.players[target].role == Role::Wolf;
                self.push(Event::SeerLook { player: p, target, wolf, thought }, emit);
            } else {
                let (thought, save, poison) = match action {
                    Action::Potion { thought, save, poison } => (thought, save, poison),
                    Action::Speak { thought, .. } | Action::Choose { thought, .. } => (thought, false, None),
                };
                let save = save && self.heal_left && victim.is_some();
                let poison = poison.filter(|&t| self.poison_left && t != p && self.alive[t]);
                if save {
                    self.heal_left = false;
                    saved = true;
                }
                if poison.is_some() {
                    self.poison_left = false;
                    poisoned = poison;
                }
                self.push(Event::Witch { player: p, victim, save, poison, thought }, emit);
            }
        }

        let mut deaths = Vec::new();
        if let Some(v) = victim.filter(|_| !saved) {
            deaths.push(v);
        }
        if let Some(p) = poisoned.filter(|p| !deaths.contains(p)) {
            deaths.push(p);
        }
        for &d in &deaths {
            self.alive[d] = false;
        }
        self.push(Event::Dawn { day, deaths }, emit);
        Ok(())
    }

    fn day(&mut self, day: u32, emit: Emit) -> Result<(), String> {
        let mut order = self.alive_list();
        let shift = (day as usize - 1) % order.len().max(1);
        order.rotate_left(shift);
        for round in 1..=2 {
            for &p in &order {
                self.push(Event::Thinking { player: p }, emit);
                let (thought, say) = speech_of(self.ask(p, Task::Speak { day, round }, emit)?);
                self.push(Event::Speech { player: p, round, thought, say }, emit);
            }
        }

        let alive = self.alive_list();
        let requests = alive.iter().map(|&p| (p, Task::Vote { day, targets: alive.iter().copied().filter(|&q| q != p).collect() })).collect();
        let mut ballots = self.ask_many(requests, emit)?;
        ballots.sort_by_key(|(p, _)| *p);
        let mut tally = vec![0u32; self.players.len()];
        for (p, action) in ballots {
            let targets: Vec<usize> = alive.iter().copied().filter(|&q| q != p).collect();
            let (thought, wanted) = choice_of(action);
            let target = self.valid_target(wanted, &targets);
            tally[target] += 1;
            self.push(Event::Vote { player: p, target, thought }, emit);
        }
        let top = *tally.iter().max().unwrap_or(&0);
        let leaders: Vec<usize> = (0..tally.len()).filter(|&p| tally[p] == top && top > 0).collect();
        let eliminated = if leaders.len() == 1 { Some(leaders[0]) } else { None };
        self.push(Event::Verdict { eliminated, tally }, emit);
        if let Some(p) = eliminated {
            self.alive[p] = false;
            let (thought, say) = speech_of(self.ask(p, Task::LastWords, emit)?);
            self.push(Event::LastWords { player: p, thought, say }, emit);
        }
        Ok(())
    }
}

fn majority(picks: &[usize]) -> Option<usize> {
    let first = *picks.first()?;
    picks.iter().copied().max_by_key(|p| (picks.iter().filter(|q| *q == p).count(), *p == first))
}

fn speech_of(action: Action) -> (String, String) {
    match action {
        Action::Speak { thought, say } => (thought, say),
        Action::Choose { thought, .. } | Action::Potion { thought, .. } => (thought, String::new()),
    }
}

fn choice_of(action: Action) -> (String, Option<usize>) {
    match action {
        Action::Choose { thought, target } => (thought, target),
        Action::Speak { thought, .. } | Action::Potion { thought, .. } => (thought, None),
    }
}

fn fallback(task: &Task) -> Action {
    match task {
        Task::WolfTalk { .. } | Task::Speak { .. } | Task::LastWords => Action::Speak { thought: String::new(), say: "...".into() },
        Task::Witch { .. } => Action::Potion { thought: String::new(), save: false, poison: None },
        Task::WolfPick { .. } | Task::Seer { .. } | Task::Vote { .. } => Action::Choose { thought: String::new(), target: None },
    }
}

pub struct ScriptedAgent {
    rng: Rng,
    me: usize,
}

impl ScriptedAgent {
    pub fn new(seed: u64, me: usize) -> Self {
        Self { rng: Rng::new(seed.wrapping_add(me as u64 * 7919)), me }
    }
}

impl Agent for ScriptedAgent {
    fn act(&mut self, news: &[String], task: &Task) -> Result<Reply, String> {
        let action = match task {
            Task::WolfTalk { .. } | Task::Speak { .. } | Task::LastWords => {
                Action::Speak { thought: format!("seat {} read {} lines", self.me, news.len()), say: format!("seat {} speaks", self.me) }
            }
            Task::WolfPick { targets, .. } | Task::Seer { targets, .. } | Task::Vote { targets, .. } => {
                Action::Choose { thought: String::new(), target: self.rng.pick(targets) }
            }
            Task::Witch { can_save, can_poison, targets, .. } => Action::Potion {
                thought: String::new(),
                save: *can_save && self.rng.below(2) == 0,
                poison: if *can_poison && self.rng.below(3) == 0 { self.rng.pick(targets) } else { None },
            },
        };
        Ok(Reply { action: Some(action), usage: Usage::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::lineup;

    fn play(seed: u64) -> (Vec<Event>, Option<Team>, Vec<PlayerInfo>) {
        let players = deal(seed, Lang::Fr, &lineup("mixed").unwrap());
        let agents: Vec<Box<dyn Agent>> = (0..SEATS).map(|i| Box::new(ScriptedAgent::new(seed, i)) as Box<dyn Agent>).collect();
        let engine = Engine::new(seed, Lang::Fr, players.clone(), agents);
        let mut events = Vec::new();
        let winner = engine.run(&mut |e: &Event| events.push(e.clone()));
        (events, winner, players)
    }

    #[test]
    fn every_seed_ends_with_a_consistent_winner() {
        for seed in 0..300 {
            let (events, winner, players) = play(seed);
            let winner = winner.expect("scripted games never abort");
            let mut alive = [true; SEATS];
            for e in &events {
                match e {
                    Event::Dawn { deaths, .. } => deaths.iter().for_each(|&d| alive[d] = false),
                    Event::Verdict { eliminated: Some(p), .. } => alive[*p] = false,
                    _ => {}
                }
            }
            let wolves = (0..SEATS).filter(|&p| alive[p] && players[p].role == Role::Wolf).count();
            let others = (0..SEATS).filter(|&p| alive[p]).count() - wolves;
            match winner {
                Team::Village => assert_eq!(wolves, 0, "seed {seed}"),
                Team::Wolves => assert!(wolves >= others || wolves > 0, "seed {seed}"),
            }
            assert!(matches!(events.last(), Some(Event::GameOver { .. })));
        }
    }

    #[test]
    fn the_dead_never_speak_or_vote() {
        for seed in 0..200 {
            let (events, _, _) = play(seed);
            let mut alive = [true; SEATS];
            for e in &events {
                match e {
                    Event::Speech { player, .. } | Event::Vote { player, .. } | Event::WolfTalk { player, .. } | Event::WolfPick { player, .. } => {
                        assert!(alive[*player], "seed {seed}: dead seat {player} acted")
                    }
                    Event::Dawn { deaths, .. } => deaths.iter().for_each(|&d| alive[d] = false),
                    Event::Verdict { eliminated: Some(p), .. } => alive[*p] = false,
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn wolves_never_eat_wolves_and_nobody_votes_for_themselves() {
        for seed in 0..200 {
            let (events, _, players) = play(seed);
            for e in &events {
                match e {
                    Event::WolfPick { target, .. } | Event::Pack { target } => {
                        assert_ne!(players[*target].role, Role::Wolf)
                    }
                    Event::Vote { player, target, .. } => assert_ne!(player, target),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn each_potion_is_used_at_most_once() {
        for seed in 0..300 {
            let (events, _, _) = play(seed);
            let saves = events.iter().filter(|e| matches!(e, Event::Witch { save: true, .. })).count();
            let poisons = events.iter().filter(|e| matches!(e, Event::Witch { poison: Some(_), .. })).count();
            assert!(saves <= 1 && poisons <= 1, "seed {seed}");
        }
    }

    #[test]
    fn deal_gives_two_wolves_one_seer_one_witch() {
        for seed in 0..50 {
            let players = deal(seed, Lang::En, &lineup("mixed").unwrap());
            let count = |r: Role| players.iter().filter(|p| p.role == r).count();
            assert_eq!((count(Role::Wolf), count(Role::Seer), count(Role::Witch), count(Role::Villager)), (2, 1, 1, 4));
            let mut names: Vec<&str> = players.iter().map(|p| p.name.as_str()).collect();
            names.sort();
            names.dedup();
            assert_eq!(names.len(), SEATS);
        }
    }

    #[test]
    fn villagers_never_hear_the_wolves_or_the_seer() {
        let players = deal(1, Lang::Fr, &lineup("mixed").unwrap());
        let villager = players.iter().position(|p| p.role == Role::Villager).unwrap();
        let wolf = players.iter().position(|p| p.role == Role::Wolf).unwrap();
        let seer = players.iter().position(|p| p.role == Role::Seer).unwrap();
        let secret = [
            Event::WolfTalk { player: wolf, thought: "t".into(), say: "s".into() },
            Event::Pack { target: villager },
            Event::SeerLook { player: seer, target: wolf, wolf: true, thought: "t".into() },
        ];
        for e in &secret {
            assert!(!e.visible_to(villager, &players), "{e:?}");
        }
        assert!(secret[0].visible_to(wolf, &players));
        assert!(secret[2].visible_to(seer, &players));
        assert!(Event::Speech { player: wolf, round: 1, thought: "secret".into(), say: "x".into() }.visible_to(villager, &players));
    }

    #[test]
    fn majority_breaks_ties_with_the_first_pick() {
        assert_eq!(majority(&[3, 5]), Some(3));
        assert_eq!(majority(&[3, 5, 5]), Some(5));
        assert_eq!(majority(&[]), None);
    }
}
