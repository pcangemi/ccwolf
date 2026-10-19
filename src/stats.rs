use std::collections::HashMap;
use std::path::Path;

use crate::game::{Event, PlayerInfo, Role, Team};
use crate::lang::Lang;
use crate::model::Model;

#[derive(Default, Debug)]
pub struct Line {
    pub seats: u32,
    pub wolf_seats: u32,
    pub wolf_wins: u32,
    pub wolves_caught: u32,
    pub village_seats: u32,
    pub village_wins: u32,
    pub village_votes: u32,
    pub votes_on_wolves: u32,
    pub survived: u32,
    pub cost: f64,
    pub wolf_lies: Vec<f32>,
    pub wolf_convincing: Vec<f32>,
    pub village_lies: Vec<f32>,
}

pub fn tally(games: &[Vec<Event>]) -> HashMap<Model, Line> {
    let mut lines: HashMap<Model, Line> = HashMap::new();
    for events in games {
        let Some(Event::Setup { players, .. }) = events.first() else {
            continue;
        };
        let Some(winner) = events.iter().find_map(|e| match e {
            Event::GameOver { winner } => Some(*winner),
            _ => None,
        }) else {
            continue;
        };
        let mut alive = vec![true; players.len()];
        for e in events {
            match e {
                Event::Dawn { deaths, .. } => deaths.iter().for_each(|&d| alive[d] = false),
                Event::Verdict { eliminated: Some(p), .. } => {
                    alive[*p] = false;
                    if players[*p].role == Role::Wolf {
                        lines.entry(players[*p].model).or_default().wolves_caught += 1;
                    }
                }
                Event::Vote { player, target, .. } if players[*player].role != Role::Wolf => {
                    let line = lines.entry(players[*player].model).or_default();
                    line.village_votes += 1;
                    if players[*target].role == Role::Wolf {
                        line.votes_on_wolves += 1;
                    }
                }
                Event::Usage { player, cost, .. } => lines.entry(players[*player].model).or_default().cost += cost,
                Event::Speech { player, verdict: Some(v), .. } => {
                    let line = lines.entry(players[*player].model).or_default();
                    if players[*player].role == Role::Wolf {
                        line.wolf_lies.push(v.lie);
                        line.wolf_convincing.extend(v.convincing);
                    } else {
                        line.village_lies.push(v.lie);
                    }
                }
                _ => {}
            }
        }
        for (i, p) in players.iter().enumerate() {
            seat(lines.entry(p.model).or_default(), p, winner, alive[i]);
        }
    }
    lines
}

fn seat(line: &mut Line, p: &PlayerInfo, winner: Team, alive: bool) {
    line.seats += 1;
    line.survived += alive as u32;
    if p.role == Role::Wolf {
        line.wolf_seats += 1;
        line.wolf_wins += (winner == Team::Wolves) as u32;
    } else {
        line.village_seats += 1;
        line.village_wins += (winner == Team::Village) as u32;
    }
}

fn mean(values: &[f32]) -> Option<f32> {
    (!values.is_empty()).then(|| values.iter().sum::<f32>() / values.len() as f32)
}

fn pct(a: u32, b: u32) -> String {
    if b == 0 { "-".into() } else { format!("{:.0}%", 100.0 * a as f64 / b as f64) }
}

pub fn print(dir: &Path, lang: Lang) -> Result<(), String> {
    let mut games = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|x| x == "jsonl")
            && let Ok(events) = crate::record::load(&path)
        {
            games.push(events);
        }
    }
    let finished = games.iter().filter(|g| g.iter().any(|e| matches!(e, Event::GameOver { .. }))).count();
    let lines = tally(&games);
    let fr = lang == Lang::Fr;
    println!("{} {}", finished, if fr { "parties terminées" } else { "finished games" });
    println!();
    let head = if fr {
        ["Modèle", "Places", "Loup gagne", "Loup démasqué", "Village gagne", "Vote juste", "Survit", "Coût/place"]
    } else {
        ["Model", "Seats", "Wolf wins", "Wolf caught", "Village wins", "Right vote", "Survives", "Cost/seat"]
    };
    println!("{:<12}{:>8}{:>13}{:>15}{:>15}{:>12}{:>10}{:>12}", head[0], head[1], head[2], head[3], head[4], head[5], head[6], head[7]);
    for model in Model::ALL {
        let Some(l) = lines.get(&model) else { continue };
        println!(
            "{:<12}{:>8}{:>13}{:>15}{:>15}{:>12}{:>10}{:>12}",
            model.label(),
            l.seats,
            pct(l.wolf_wins, l.wolf_seats),
            pct(l.wolves_caught, l.wolf_seats),
            pct(l.village_wins, l.village_seats),
            pct(l.votes_on_wolves, l.village_votes),
            pct(l.survived, l.seats),
            lang.money(if l.seats == 0 { 0.0 } else { l.cost / l.seats as f64 }),
        );
    }
    if lines.values().any(|l| !l.wolf_lies.is_empty() || !l.village_lies.is_empty()) {
        println!();
        println!("{}", if fr { "Détecteur de mensonges Jev, moyenne par prise de parole" } else { "Jev lie detector, average per speech" });
        let head = if fr {
            ["Modèle", "Mensonge en loup", "Convaincant en loup", "Bluff en villageois"]
        } else {
            ["Model", "Lies as wolf", "Convincing as wolf", "Bluffs as villager"]
        };
        println!("{:<12}{:>18}{:>22}{:>22}", head[0], head[1], head[2], head[3]);
        let pct = |v: Option<f32>| v.map(|x| format!("{:.0}%", x * 100.0)).unwrap_or_else(|| "-".into());
        for model in Model::ALL {
            let Some(l) = lines.get(&model) else { continue };
            let convincing = mean(&l.wolf_convincing).map(|c| format!("{c:.1}/5")).unwrap_or_else(|| "-".into());
            println!("{:<12}{:>18}{:>22}{:>22}", model.label(), pct(mean(&l.wolf_lies)), convincing, pct(mean(&l.village_lies)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Agent, Engine, SEATS, ScriptedAgent, deal};
    use crate::model::lineup;

    #[test]
    fn every_seat_of_every_finished_game_is_counted_once() {
        let games: Vec<Vec<Event>> = (0..20)
            .map(|seed| {
                let players = deal(seed, Lang::En, &lineup("mixed").unwrap());
                let agents: Vec<Box<dyn Agent>> = (0..SEATS).map(|i| Box::new(ScriptedAgent::new(seed, i)) as Box<dyn Agent>).collect();
                let mut events = Vec::new();
                Engine::new(seed, Lang::En, players, agents).run(&mut |e: &Event| events.push(e.clone()));
                events
            })
            .collect();
        let lines = tally(&games);
        let seats: u32 = lines.values().map(|l| l.seats).sum();
        let wolves: u32 = lines.values().map(|l| l.wolf_seats).sum();
        assert_eq!(seats, 20 * SEATS as u32);
        assert_eq!(wolves, 40);
        for l in lines.values() {
            assert!(l.votes_on_wolves <= l.village_votes);
            assert!(l.wolves_caught <= l.wolf_seats);
        }
    }
}
