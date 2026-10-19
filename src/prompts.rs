use serde_json::{Value, json};

use crate::game::{Event, PlayerInfo, Role, Task};
use crate::lang::Lang;

const SYSTEM_FR: &str = include_str!("../prompts/fr.txt");
const SYSTEM_EN: &str = include_str!("../prompts/en.txt");

pub fn quote(lang: Lang, text: &str) -> String {
    match lang {
        Lang::Fr => format!("«\u{a0}{}\u{a0}»", text.trim()),
        Lang::En => format!("\u{201c}{}\u{201d}", text.trim()),
    }
}

fn list(lang: Lang, players: &[PlayerInfo], ids: &[usize]) -> String {
    let names: Vec<&str> = ids.iter().map(|&i| players[i].name.as_str()).collect();
    match names.len() {
        0 => String::new(),
        1 => names[0].to_string(),
        n => {
            let and = if lang == Lang::Fr { "et" } else { "and" };
            format!("{} {} {}", names[..n - 1].join(", "), and, names[n - 1])
        }
    }
}

pub fn nobody(lang: Lang) -> &'static str {
    if lang == Lang::Fr { "personne" } else { "nobody" }
}

fn role_card(lang: Lang, me: usize, players: &[PlayerInfo]) -> String {
    let partners: Vec<usize> = (0..players.len()).filter(|&p| p != me && players[p].role == Role::Wolf).collect();
    let partner = list(lang, players, &partners);
    match (lang, players[me].role) {
        (Lang::Fr, Role::Wolf) => format!(
            "Ton rôle secret : LOUP-GAROU, avec {partner}. La nuit, vous choisissez ensemble une victime. Le jour, fais-toi passer pour un simple villageois : détourne les soupçons, accuse les autres, protège ton complice sans en avoir l'air."
        ),
        (Lang::Fr, Role::Seer) => "Ton rôle secret : VOYANTE. Chaque nuit, tu découvres si un joueur est loup-garou. Partage tes découvertes au bon moment : si les loups te démasquent, tu seras leur prochaine victime.".into(),
        (Lang::Fr, Role::Witch) => "Ton rôle secret : SORCIÈRE. Chaque nuit, on te dit qui les loups ont attaqué. Tu as une potion de vie et une potion de mort, chacune utilisable une seule fois dans la partie.".into(),
        (Lang::Fr, Role::Villager) => "Ton rôle secret : SIMPLE VILLAGEOIS. Tu n'as aucun pouvoir : écoute, repère les contradictions et démasque les loups-garous.".into(),
        (Lang::En, Role::Wolf) => format!(
            "Your secret role: WEREWOLF, together with {partner}. At night, you pick a victim together. By day, pass yourself off as a plain villager: deflect suspicion, accuse others, protect your partner without looking like it."
        ),
        (Lang::En, Role::Seer) => "Your secret role: SEER. Each night, you learn whether one player is a werewolf. Share what you learn at the right moment: if the werewolves unmask you, you will be their next victim.".into(),
        (Lang::En, Role::Witch) => "Your secret role: WITCH. Each night, you are told who the werewolves attacked. You have a healing potion and a poison, each usable once per game.".into(),
        (Lang::En, Role::Villager) => "Your secret role: PLAIN VILLAGER. You have no power: listen, spot contradictions and unmask the werewolves.".into(),
    }
}

pub fn system(lang: Lang, me: usize, players: &[PlayerInfo]) -> String {
    let template = if lang == Lang::Fr { SYSTEM_FR } else { SYSTEM_EN };
    let all: Vec<usize> = (0..players.len()).collect();
    template
        .replace("{name}", &players[me].name)
        .replace("{persona}", &players[me].persona)
        .replace("{players}", &list(lang, players, &all))
        .replace("{role_card}", &role_card(lang, me, players))
}

pub fn schema(lang: Lang, players: &[PlayerInfo]) -> Value {
    let names: Vec<&str> = players.iter().map(|p| p.name.as_str()).collect();
    let mut with_nobody = names.clone();
    with_nobody.push(nobody(lang));
    let fr = lang == Lang::Fr;
    json!({
        "type": "object",
        "properties": {
            "aside": { "type": "string", "description": if fr { "L'aparté de ton personnage : son monologue intérieur, que personne d'autre n'entend (30 mots maximum)." } else { "Your character's aside: their inner monologue, which nobody else hears (30 words at most)." } },
            "speech": { "type": "string", "description": if fr { "Quand on te demande de parler : ce que ton personnage dit à voix haute (60 mots maximum)." } else { "When asked to speak: what your character says out loud (60 words at most)." } },
            "target": { "type": "string", "enum": names, "description": if fr { "Quand on te demande de choisir ou de voter : le joueur choisi." } else { "When asked to choose or vote: the chosen player." } },
            "save": { "type": "boolean", "description": if fr { "Sorcière : vrai pour sauver la victime des loups avec la potion de vie." } else { "Witch: true to save the werewolves' victim with the healing potion." } },
            "poison": { "type": "string", "enum": with_nobody, "description": if fr { "Sorcière : le joueur à empoisonner, ou personne." } else { "Witch: the player to poison, or nobody." } }
        },
        "required": ["aside"],
        "additionalProperties": false
    })
}

fn fields(lang: Lang, task: &Task) -> &'static str {
    match (lang, task) {
        (Lang::Fr, Task::WolfTalk { .. } | Task::Speak { .. } | Task::LastWords) => "Remplis aside, puis speech.",
        (Lang::Fr, Task::WolfPick { .. } | Task::Seer { .. } | Task::Vote { .. }) => "Remplis aside, puis target.",
        (Lang::Fr, Task::Witch { .. }) => "Remplis aside, puis save et poison.",
        (Lang::En, Task::WolfTalk { .. } | Task::Speak { .. } | Task::LastWords) => "Fill in aside, then speech.",
        (Lang::En, Task::WolfPick { .. } | Task::Seer { .. } | Task::Vote { .. }) => "Fill in aside, then target.",
        (Lang::En, Task::Witch { .. }) => "Fill in aside, then save and poison.",
    }
}

pub fn task_text(lang: Lang, task: &Task, players: &[PlayerInfo]) -> String {
    format!("{} {}", situation(lang, task, players), fields(lang, task))
}

fn situation(lang: Lang, task: &Task, players: &[PlayerInfo]) -> String {
    let fr = lang == Lang::Fr;
    match task {
        Task::WolfTalk { day, targets } => {
            let t = list(lang, players, targets);
            if fr {
                format!("Nuit {day}. Conciliabule des loups : propose à ton complice une victime pour cette nuit et dis pourquoi. Victimes possibles : {t}.")
            } else {
                format!("Night {day}. Werewolf council: suggest a victim for tonight to your partner and say why. Possible victims: {t}.")
            }
        }
        Task::WolfPick { targets, .. } => {
            let t = list(lang, players, targets);
            if fr { format!("Choisis la victime de cette nuit parmi : {t}.") } else { format!("Pick tonight's victim among: {t}.") }
        }
        Task::Seer { day, targets } => {
            let t = list(lang, players, targets);
            if fr {
                format!("Nuit {day}. Choisis le joueur dont tu veux découvrir le rôle parmi : {t}.")
            } else {
                format!("Night {day}. Choose the player whose role you want to learn among: {t}.")
            }
        }
        Task::Witch { day, victim, can_save, can_poison, targets } => {
            let t = list(lang, players, targets);
            let attacked = match (victim, fr) {
                (Some(v), true) => format!("Les loups ont attaqué {}.", players[*v].name),
                (Some(v), false) => format!("The werewolves attacked {}.", players[*v].name),
                (None, true) => "Les loups n'ont attaqué personne.".into(),
                (None, false) => "The werewolves attacked nobody.".into(),
            };
            let heal = match (can_save, fr) {
                (true, true) => "Tu peux encore sauver la victime avec ta potion de vie.",
                (true, false) => "You can still save the victim with your healing potion.",
                (false, true) => "Tu ne peux sauver personne : réponds faux pour save.",
                (false, false) => "You cannot save anyone: answer false for save.",
            };
            let poison = match (can_poison, fr) {
                (true, true) => {
                    format!("Tu peux aussi empoisonner un joueur parmi : {t}, ou personne.")
                }
                (true, false) => format!("You can also poison one player among: {t}, or nobody."),
                (false, true) => "Ta potion de mort est épuisée : réponds personne pour poison.".into(),
                (false, false) => "Your poison is used up: answer nobody for poison.".into(),
            };
            if fr { format!("Nuit {day}. {attacked} {heal} {poison}") } else { format!("Night {day}. {attacked} {heal} {poison}") }
        }
        Task::Speak { day, round: 1 } => {
            if fr {
                format!("Jour {day}. C'est ton tour de parler devant le village.")
            } else {
                format!("Day {day}. It is your turn to speak to the village.")
            }
        }
        Task::Speak { day, .. } => {
            if fr {
                format!("Jour {day}, second tour de parole : réagis à ce qui vient d'être dit, le vote approche.")
            } else {
                format!("Day {day}, second round: react to what was just said, the vote is coming.")
            }
        }
        Task::Vote { day, targets } => {
            let t = list(lang, players, targets);
            if fr { format!("Jour {day}. Vote pour éliminer un joueur parmi : {t}.") } else { format!("Day {day}. Vote to eliminate one player among: {t}.") }
        }
        Task::LastWords => {
            if fr {
                "Le village vient de t'éliminer. Tes dernières paroles ?".into()
            } else {
                "The village has just eliminated you. Your last words?".into()
            }
        }
    }
}

fn who(lang: Lang, p: usize, viewer: usize, players: &[PlayerInfo]) -> String {
    if p == viewer { format!("{} ({})", players[p].name, if lang == Lang::Fr { "toi" } else { "you" }) } else { players[p].name.clone() }
}

fn with_role(lang: Lang, p: usize, players: &[PlayerInfo]) -> String {
    let role = lang.ui().role(players[p].role);
    match lang {
        Lang::Fr => format!("{} (rôle : {role})", players[p].name),
        Lang::En => format!("{} (role: {role})", players[p].name),
    }
}

pub fn describe(lang: Lang, e: &Event, viewer: usize, players: &[PlayerInfo]) -> Option<String> {
    let fr = lang == Lang::Fr;
    let name = |p: usize| who(lang, p, viewer, players);
    let q = |s: &str| quote(lang, s);
    Some(match e {
        Event::Night { day } => {
            if fr {
                format!("La nuit {day} tombe sur le village.")
            } else {
                format!("Night {day} falls on the village.")
            }
        }
        Event::WolfTalk { player, say, .. } => {
            if fr {
                format!("(Loups) {} : {}", name(*player), q(say))
            } else {
                format!("(Werewolves) {}: {}", name(*player), q(say))
            }
        }
        Event::WolfPick { player, target, .. } => {
            if fr {
                format!("(Loups) {} veut dévorer {}.", name(*player), name(*target))
            } else {
                format!("(Werewolves) {} wants to devour {}.", name(*player), name(*target))
            }
        }
        Event::Pack { target } => {
            if fr {
                format!("(Loups) La meute dévorera {} cette nuit.", name(*target))
            } else {
                format!("(Werewolves) The pack will devour {} tonight.", name(*target))
            }
        }
        Event::SeerLook { target, wolf, .. } => match (fr, wolf) {
            (true, true) => format!("Ta vision : {} est un loup-garou.", name(*target)),
            (true, false) => format!("Ta vision : {} n'est pas un loup-garou.", name(*target)),
            (false, true) => format!("Your vision: {} is a werewolf.", name(*target)),
            (false, false) => format!("Your vision: {} is not a werewolf.", name(*target)),
        },
        Event::Witch { victim, save, poison, .. } => {
            let mut parts = Vec::new();
            if let (true, Some(v)) = (save, victim) {
                parts.push(if fr {
                    format!("Tu as sauvé {} avec ta potion de vie.", name(*v))
                } else {
                    format!("You saved {} with your healing potion.", name(*v))
                });
            }
            if let Some(p) = poison {
                parts.push(if fr { format!("Tu as empoisonné {}.", name(*p)) } else { format!("You poisoned {}.", name(*p)) });
            }
            if parts.is_empty() {
                parts.push(if fr { "Tu n'as utilisé aucune potion cette nuit.".into() } else { "You used no potion tonight.".into() });
            }
            parts.join(" ")
        }
        Event::Dawn { day, deaths } => {
            if deaths.is_empty() {
                if fr { format!("Le jour {day} se lève : personne n'est mort cette nuit.") } else { format!("Day {day} dawns: nobody died last night.") }
            } else {
                let dead: Vec<String> = deaths.iter().map(|&d| with_role(lang, d, players)).collect();
                let dead = dead.join(if fr { " et " } else { " and " });
                if fr {
                    format!("Le jour {day} se lève. {} de la nuit : {dead}.", if deaths.len() > 1 { "Victimes" } else { "Victime" })
                } else {
                    format!("Day {day} dawns. {} of the night: {dead}.", if deaths.len() > 1 { "Victims" } else { "Victim" })
                }
            }
        }
        Event::Speech { player, say, .. } => format!("{}{} {}", name(*player), if fr { " :" } else { ":" }, q(say)),
        Event::Vote { player, target, .. } => {
            if fr {
                format!("{} vote contre {}.", name(*player), name(*target))
            } else {
                format!("{} votes against {}.", name(*player), name(*target))
            }
        }
        Event::Verdict { eliminated: Some(p), .. } => {
            if fr {
                format!("Le village élimine {}.", with_role(lang, *p, players))
            } else {
                format!("The village eliminates {}.", with_role(lang, *p, players))
            }
        }
        Event::Verdict { eliminated: None, .. } => {
            if fr {
                "Égalité : personne n'est éliminé aujourd'hui.".into()
            } else {
                "Tie: nobody is eliminated today.".into()
            }
        }
        Event::LastWords { player, say, .. } => {
            if fr {
                format!("Dernières paroles de {} : {}", name(*player), q(say))
            } else {
                format!("Last words of {}: {}", name(*player), q(say))
            }
        }
        Event::Setup { .. } | Event::Thinking { .. } | Event::Usage { .. } | Event::GameOver { .. } | Event::Aborted { .. } => return None,
    })
}

fn jev_line(lang: Lang, v: &crate::game::Verdict, players: &[PlayerInfo]) -> String {
    let ui = lang.ui();
    let mut parts = vec![format!("{} {:.0}{}", ui.lie, v.lie * 100.0, if lang == Lang::Fr { "\u{a0}%" } else { "%" })];
    if let Some(c) = v.convincing {
        parts.push(format!("{}/5", lang.decimal(c)));
    }
    if let Some(role) = v.claim {
        parts.push(format!("{} {}", ui.claims, ui.role(role)));
    }
    if let Some(a) = v.accuses {
        parts.push(format!("{} {}", if lang == Lang::Fr { "accuse" } else { "accuses" }, players[a].name));
    }
    format!("    jev: {}", parts.join(", "))
}

pub fn narrate(lang: Lang, e: &Event, players: &[PlayerInfo]) -> Vec<String> {
    let fr = lang == Lang::Fr;
    let tag = |p: usize| format!("{} [{}, {}]", players[p].name, players[p].model.label(), lang.ui().role(players[p].role));
    let thought = |t: &str| {
        if t.trim().is_empty() { None } else { Some(format!("    {} {}", if fr { "pensée :" } else { "thought:" }, t.trim())) }
    };
    let mut lines = Vec::new();
    match e {
        Event::Setup { seed, players: ps, .. } => {
            lines.push(format!("ccwolf, seed {seed}"));
            for (p, info) in ps.iter().enumerate() {
                lines.push(format!("  {}, {}", tag(p), info.persona));
            }
        }
        Event::WolfTalk { player, thought: t, .. } | Event::Speech { player, thought: t, .. } | Event::LastWords { player, thought: t, .. } => {
            let mut line = describe(lang, e, usize::MAX, players).unwrap_or_default();
            line = line.replacen(&players[*player].name, &tag(*player), 1);
            lines.push(line);
            lines.extend(thought(t));
            if let Event::Speech { verdict: Some(v), .. } | Event::LastWords { verdict: Some(v), .. } = e {
                lines.push(jev_line(lang, v, players));
            }
        }
        Event::WolfPick { thought: t, .. } | Event::Vote { thought: t, .. } => {
            lines.extend(describe(lang, e, usize::MAX, players));
            lines.extend(thought(t));
        }
        Event::SeerLook { player, target, wolf, thought: t } => {
            let verdict = match (fr, wolf) {
                (true, true) => "LOUP-GAROU",
                (true, false) => "innocent",
                (false, true) => "WEREWOLF",
                (false, false) => "innocent",
            };
            lines.push(format!("{} -> {} : {verdict}", tag(*player), players[*target].name));
            lines.extend(thought(t));
        }
        Event::Witch { player, thought: t, .. } => {
            lines.push(format!("{} : {}", tag(*player), describe(lang, e, *player, players).unwrap_or_default()));
            lines.extend(thought(t));
        }
        Event::Usage { .. } | Event::Thinking { .. } => {}
        Event::GameOver { winner } => lines.push(lang.ui().winner(*winner).to_string()),
        Event::Aborted { reason } => lines.push(format!("{} : {reason}", lang.ui().aborted)),
        _ => lines.extend(describe(lang, e, usize::MAX, players)),
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deal;
    use crate::model::lineup;

    fn table(lang: Lang) -> Vec<PlayerInfo> {
        deal(11, lang, &lineup("mixed").unwrap())
    }

    #[test]
    fn prompts_have_no_em_dash_and_no_leftover_placeholder() {
        for lang in [Lang::Fr, Lang::En] {
            let players = table(lang);
            for me in 0..players.len() {
                let s = system(lang, me, &players);
                assert!(!s.contains('\u{2014}'));
                assert!(!s.contains('{'), "{s}");
                assert!(s.contains(&players[me].name));
            }
            assert!(!schema(lang, &players).to_string().contains('\u{2014}'));
        }
    }

    #[test]
    fn a_wolf_knows_its_partner_and_nobody_else_learns_roles() {
        let players = table(Lang::Fr);
        let wolves: Vec<usize> = (0..8).filter(|&p| players[p].role == Role::Wolf).collect();
        let card = system(Lang::Fr, wolves[0], &players);
        assert!(card.contains(&format!("LOUP-GAROU, avec {}", players[wolves[1]].name)));
        let villager = (0..8).find(|&p| players[p].role == Role::Villager).unwrap();
        let card = system(Lang::Fr, villager, &players);
        assert!(!card.contains("LOUP-GAROU, avec"));
    }

    #[test]
    fn players_hear_speeches_but_never_thoughts() {
        let players = table(Lang::En);
        let e = Event::Speech { player: 2, round: 1, thought: "I am the wolf".into(), say: "Good morning".into(), verdict: None };
        let line = describe(Lang::En, &e, 5, &players).unwrap();
        assert!(line.contains("Good morning"));
        assert!(!line.contains("I am the wolf"));
    }

    #[test]
    fn the_schema_lists_every_player_and_nobody_for_the_poison() {
        let players = table(Lang::Fr);
        let schema = schema(Lang::Fr, &players);
        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], json!(["aside"]));
        let props = &schema["properties"];
        assert!(props.get("thought").is_none(), "a field named thought makes Opus and Fable refuse");
        assert_eq!(props["target"]["enum"].as_array().unwrap().len(), 8);
        let poison = props["poison"]["enum"].as_array().unwrap();
        assert_eq!(poison.len(), 9);
        assert_eq!(poison[8], "personne");
    }

    #[test]
    fn every_task_says_which_fields_to_fill() {
        let players = table(Lang::En);
        let tasks = [
            Task::WolfTalk { day: 1, targets: vec![0, 1] },
            Task::WolfPick { day: 1, targets: vec![0] },
            Task::Seer { day: 1, targets: vec![2] },
            Task::Witch { day: 1, victim: Some(3), can_save: true, can_poison: true, targets: vec![1, 2] },
            Task::Speak { day: 2, round: 1 },
            Task::Speak { day: 2, round: 2 },
            Task::Vote { day: 2, targets: vec![4, 5] },
            Task::LastWords,
        ];
        for task in &tasks {
            let text = task_text(Lang::En, task, &players);
            assert!(text.contains("Fill in aside"), "{text}");
            assert!(!task_text(Lang::Fr, task, &players).contains('\u{2014}'));
        }
    }
}
