use ratatui::crossterm::event::KeyCode;

use crate::game::{Action, Task};
use crate::lang::Lang;

pub const NOBODY: usize = usize::MAX;
const MAX_SPEECH: usize = 400;

#[derive(Clone, Debug, PartialEq)]
pub enum Input {
    Speech { kind: SpeechKind, buffer: String },
    Pick { kind: PickKind, options: Vec<usize>, index: usize },
    Save { victim: usize, poison: Option<Vec<usize>>, index: usize },
    Poison { saved: bool, options: Vec<usize>, index: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpeechKind {
    Council,
    Village,
    LastWords,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PickKind {
    Devour,
    Look,
    Vote,
}

pub enum Outcome {
    Waiting,
    Next(Input),
    Done(Action),
}

impl Input {
    pub fn from_task(task: &Task) -> Result<Input, Action> {
        Ok(match task {
            Task::WolfTalk { .. } => Input::Speech { kind: SpeechKind::Council, buffer: String::new() },
            Task::Speak { .. } => Input::Speech { kind: SpeechKind::Village, buffer: String::new() },
            Task::LastWords => Input::Speech { kind: SpeechKind::LastWords, buffer: String::new() },
            Task::WolfPick { targets, .. } => Input::Pick { kind: PickKind::Devour, options: targets.clone(), index: 0 },
            Task::Seer { targets, .. } => Input::Pick { kind: PickKind::Look, options: targets.clone(), index: 0 },
            Task::Vote { targets, .. } => Input::Pick { kind: PickKind::Vote, options: targets.clone(), index: 0 },
            Task::Witch { victim, can_save, can_poison, targets, .. } => {
                let poison = can_poison.then(|| targets.iter().copied().chain([NOBODY]).collect::<Vec<_>>());
                match (victim, can_save, poison) {
                    (Some(v), true, poison) => Input::Save { victim: *v, poison, index: 0 },
                    (_, _, Some(options)) => Input::Poison { saved: false, options, index: 0 },
                    _ => return Err(Action::Potion { thought: String::new(), save: false, poison: None }),
                }
            }
        })
    }

    pub fn highlighted(&self) -> Option<usize> {
        match self {
            Input::Pick { options, index, .. } | Input::Poison { options, index, .. } => options.get(*index).copied().filter(|p| *p != NOBODY),
            Input::Save { victim, .. } => Some(*victim),
            Input::Speech { .. } => None,
        }
    }

    pub fn key(&mut self, code: KeyCode) -> Outcome {
        match self {
            Input::Speech { buffer, .. } => match code {
                KeyCode::Char(c) if buffer.chars().count() < MAX_SPEECH => {
                    buffer.push(c);
                    Outcome::Waiting
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    Outcome::Waiting
                }
                KeyCode::Enter if !buffer.trim().is_empty() => Outcome::Done(Action::Speak { thought: String::new(), say: buffer.trim().to_string() }),
                _ => Outcome::Waiting,
            },
            Input::Pick { options, index, .. } => match step(code, index, options.len()) {
                Some(()) => Outcome::Done(Action::Choose { thought: String::new(), target: options.get(*index).copied() }),
                None => Outcome::Waiting,
            },
            Input::Save { poison, index, .. } => match step(code, index, 2) {
                Some(()) => {
                    let saved = *index == 0;
                    match poison.take() {
                        Some(options) => Outcome::Next(Input::Poison { saved, options, index: 0 }),
                        None => Outcome::Done(Action::Potion { thought: String::new(), save: saved, poison: None }),
                    }
                }
                None => Outcome::Waiting,
            },
            Input::Poison { saved, options, index } => match step(code, index, options.len()) {
                Some(()) => {
                    let target = options.get(*index).copied().filter(|p| *p != NOBODY);
                    Outcome::Done(Action::Potion { thought: String::new(), save: *saved, poison: target })
                }
                None => Outcome::Waiting,
            },
        }
    }

    pub fn title(&self, lang: Lang, victim_name: &str) -> String {
        let fr = lang == Lang::Fr;
        match self {
            Input::Speech { kind: SpeechKind::Council, .. } => if fr { "À TOI : PARLE À TA MEUTE" } else { "YOUR TURN: TALK TO YOUR PACK" }.into(),
            Input::Speech { kind: SpeechKind::Village, .. } => if fr { "À TOI DE PARLER AU VILLAGE" } else { "YOUR TURN TO SPEAK TO THE VILLAGE" }.into(),
            Input::Speech { kind: SpeechKind::LastWords, .. } => if fr { "TES DERNIÈRES PAROLES" } else { "YOUR LAST WORDS" }.into(),
            Input::Pick { kind: PickKind::Devour, .. } => if fr { "QUI DÉVORER CETTE NUIT ?" } else { "WHO DO YOU DEVOUR TONIGHT?" }.into(),
            Input::Pick { kind: PickKind::Look, .. } => if fr { "QUI SONDER CETTE NUIT ?" } else { "WHOSE ROLE DO YOU LOOK AT?" }.into(),
            Input::Pick { kind: PickKind::Vote, .. } => if fr { "CONTRE QUI VOTES-TU ?" } else { "WHO DO YOU VOTE AGAINST?" }.into(),
            Input::Save { .. } => {
                if fr {
                    format!("LES LOUPS ONT ATTAQUÉ {}. LA SAUVER ?", victim_name.to_uppercase())
                } else {
                    format!("THE WEREWOLVES ATTACKED {}. SAVE THEM?", victim_name.to_uppercase())
                }
            }
            Input::Poison { .. } => if fr { "EMPOISONNER QUELQU'UN ?" } else { "POISON SOMEONE?" }.into(),
        }
    }

    pub fn hint(&self, lang: Lang) -> &'static str {
        match (self, lang) {
            (Input::Speech { .. }, Lang::Fr) => "Tape ta réplique, puis Entrée. Les IA ne savent pas qui est humain.",
            (Input::Speech { .. }, Lang::En) => "Type your line, then Enter. The AIs do not know who is human.",
            (_, Lang::Fr) => "Flèches pour choisir, Entrée pour valider.",
            (_, Lang::En) => "Arrows to choose, Enter to confirm.",
        }
    }
}

fn step(code: KeyCode, index: &mut usize, len: usize) -> Option<()> {
    match code {
        KeyCode::Left | KeyCode::Up | KeyCode::BackTab => *index = (*index + len - 1) % len.max(1),
        KeyCode::Right | KeyCode::Down | KeyCode::Tab => *index = (*index + 1) % len.max(1),
        KeyCode::Enter if len > 0 => return Some(()),
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vote_is_chosen_with_the_arrows() {
        let mut input = Input::from_task(&Task::Vote { day: 1, targets: vec![2, 4, 6] }).unwrap();
        assert!(matches!(input.key(KeyCode::Right), Outcome::Waiting));
        assert!(matches!(input.key(KeyCode::Right), Outcome::Waiting));
        assert_eq!(input.highlighted(), Some(6));
        assert!(matches!(input.key(KeyCode::Right), Outcome::Waiting));
        assert_eq!(input.highlighted(), Some(2));
        match input.key(KeyCode::Enter) {
            Outcome::Done(action) => assert_eq!(action, Action::Choose { thought: String::new(), target: Some(2) }),
            _ => panic!("the vote should be cast"),
        }
    }

    #[test]
    fn typing_keeps_letters_that_are_also_shortcuts() {
        let mut input = Input::from_task(&Task::Speak { day: 1, round: 1 }).unwrap();
        assert!(matches!(input.key(KeyCode::Enter), Outcome::Waiting));
        for c in "q t r c ".chars() {
            input.key(KeyCode::Char(c));
        }
        input.key(KeyCode::Backspace);
        match input.key(KeyCode::Enter) {
            Outcome::Done(Action::Speak { say, .. }) => assert_eq!(say, "q t r c"),
            _ => panic!("the speech should be sent"),
        }
    }

    #[test]
    fn the_witch_saves_then_may_poison() {
        let task = Task::Witch { day: 1, victim: Some(3), can_save: true, can_poison: true, targets: vec![1, 5] };
        let mut input = Input::from_task(&task).unwrap();
        let Outcome::Next(mut poison) = input.key(KeyCode::Enter) else { panic!("saving leads to the poison") };
        poison.key(KeyCode::Left);
        assert_eq!(poison.highlighted(), None);
        match poison.key(KeyCode::Enter) {
            Outcome::Done(action) => assert_eq!(action, Action::Potion { thought: String::new(), save: true, poison: None }),
            _ => panic!("the potions should be used"),
        }
        let empty = Task::Witch { day: 2, victim: None, can_save: false, can_poison: false, targets: vec![] };
        assert!(Input::from_task(&empty).is_err());
    }
}
