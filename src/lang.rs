use serde::{Deserialize, Serialize};

use crate::game::{Role, Team};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Fr,
    En,
}

impl Lang {
    pub fn parse(s: &str) -> Option<Lang> {
        match s.trim().to_lowercase().as_str() {
            "fr" => Some(Lang::Fr),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    pub fn from_env() -> Lang {
        for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(value) = std::env::var(var)
                && !value.is_empty()
            {
                return if value.to_lowercase().starts_with("fr") { Lang::Fr } else { Lang::En };
            }
        }
        Lang::En
    }

    pub fn ui(self) -> &'static Ui {
        match self {
            Lang::Fr => &FR,
            Lang::En => &EN,
        }
    }

    pub fn money(self, usd: f64) -> String {
        match self {
            Lang::Fr => format!("{:.2} $", usd).replace('.', ","),
            Lang::En => format!("${:.2}", usd),
        }
    }
}

pub struct Ui {
    pub characters: [(&'static str, &'static str); 10],
    pub wolf: &'static str,
    pub seer: &'static str,
    pub witch: &'static str,
    pub villager: &'static str,
    pub night: &'static str,
    pub day: &'static str,
    pub wolves_win: &'static str,
    pub village_wins: &'static str,
    pub alive: &'static str,
    pub secret_thought: &'static str,
    pub wolf_council: &'static str,
    pub village_square: &'static str,
    pub last_words: &'static str,
    pub vote: &'static str,
    pub lie_detector: &'static str,
    pub lie: &'static str,
    pub claims: &'static str,
    pub jev_on: &'static str,
    pub seer_vision: &'static str,
    pub witch_brew: &'static str,
    pub thinking: &'static str,
    pub roster: &'static str,
    pub chronicle: &'static str,
    pub press_enter: &'static str,
    pub subtitle: &'static str,
    pub lineup_hint: &'static str,
    pub keys: &'static str,
    pub paused: &'static str,
    pub too_small: &'static str,
    pub game_over_hint: &'static str,
    pub aborted: &'static str,
    pub replay_tag: &'static str,
    pub total_cost: &'static str,
    pub dead: &'static str,
    pub survived: &'static str,
}

impl Ui {
    pub fn role(&self, role: Role) -> &'static str {
        match role {
            Role::Wolf => self.wolf,
            Role::Seer => self.seer,
            Role::Witch => self.witch,
            Role::Villager => self.villager,
        }
    }

    pub fn winner(&self, team: Team) -> &'static str {
        match team {
            Team::Wolves => self.wolves_win,
            Team::Village => self.village_wins,
        }
    }
}

pub static FR: Ui = Ui {
    characters: [
        ("Marie", "la boulangère, qui sait tout sur tout le monde"),
        ("Léon", "l'ancien maire, très susceptible"),
        ("Jeanne", "l'institutrice à la retraite, d'une logique implacable"),
        ("Hugo", "le berger, nerveux et superstitieux"),
        ("Camille", "la couturière, qui adore les ragots"),
        ("Gaspard", "le forgeron, peu bavard mais franc"),
        ("Louise", "l'herboriste, un peu mystique"),
        ("Basile", "l'aubergiste, bavard et charmeur"),
        ("Rose", "la meunière, gentille mais crédule"),
        ("Victor", "le garde champêtre, soupçonneux de nature"),
    ],
    wolf: "loup-garou",
    seer: "voyante",
    witch: "sorcière",
    villager: "villageois",
    night: "NUIT",
    day: "JOUR",
    wolves_win: "LES LOUPS GAGNENT",
    village_wins: "LE VILLAGE GAGNE",
    alive: "vivants",
    secret_thought: "PENSÉE SECRÈTE",
    wolf_council: "CONCILIABULE DES LOUPS",
    village_square: "PLACE DU VILLAGE",
    last_words: "DERNIÈRES PAROLES",
    vote: "LE VOTE",
    lie_detector: "DÉTECTEUR JEV",
    lie: "mensonge",
    claims: "se dit",
    jev_on: "Détecteur de mensonges : Jev",
    seer_vision: "VISION DE LA VOYANTE",
    witch_brew: "CHAUDRON DE LA SORCIÈRE",
    thinking: "réfléchit",
    roster: "JOUEURS",
    chronicle: "CHRONIQUE",
    press_enter: "ENTRÉE POUR COMMENCER",
    subtitle: "Haiku, Sonnet, Opus et Fable jouent au loup-garou. Qui ment le mieux ?",
    lineup_hint: "Joueurs",
    keys: "[t] pensées  [r] rôles  [c] crt  [espace] pause  [→] passer  [+/-] vitesse  [q] quitter",
    paused: "PAUSE",
    too_small: "Agrandis ton terminal (100 x 30 minimum)",
    game_over_hint: "[q] quitter",
    aborted: "Partie interrompue",
    replay_tag: "REPLAY",
    total_cost: "Coût de la partie",
    dead: "mort",
    survived: "vivant",
};

pub static EN: Ui = Ui {
    characters: [
        ("Mary", "the baker, who knows everything about everyone"),
        ("Leo", "the former mayor, very touchy"),
        ("Jane", "the retired schoolteacher, ruthlessly logical"),
        ("Hugo", "the shepherd, nervous and superstitious"),
        ("Iris", "the seamstress, who loves gossip"),
        ("Jasper", "the blacksmith, few words but blunt"),
        ("Lucy", "the herbalist, a little mystical"),
        ("Owen", "the innkeeper, chatty and charming"),
        ("Rose", "the miller, kind but gullible"),
        ("Victor", "the village constable, suspicious by nature"),
    ],
    wolf: "werewolf",
    seer: "seer",
    witch: "witch",
    villager: "villager",
    night: "NIGHT",
    day: "DAY",
    wolves_win: "WEREWOLVES WIN",
    village_wins: "THE VILLAGE WINS",
    alive: "alive",
    secret_thought: "SECRET THOUGHT",
    wolf_council: "WEREWOLF COUNCIL",
    village_square: "VILLAGE SQUARE",
    last_words: "LAST WORDS",
    vote: "THE VOTE",
    lie_detector: "JEV LIE DETECTOR",
    lie: "lie",
    claims: "claims to be",
    jev_on: "Lie detector: Jev",
    seer_vision: "SEER'S VISION",
    witch_brew: "WITCH'S CAULDRON",
    thinking: "is thinking",
    roster: "PLAYERS",
    chronicle: "CHRONICLE",
    press_enter: "PRESS ENTER TO START",
    subtitle: "Haiku, Sonnet, Opus and Fable play werewolf. Who lies best?",
    lineup_hint: "Players",
    keys: "[t] thoughts  [r] roles  [c] crt  [space] pause  [→] skip  [+/-] speed  [q] quit",
    paused: "PAUSED",
    too_small: "Make your terminal bigger (100 x 30 minimum)",
    game_over_hint: "[q] quit",
    aborted: "Game stopped",
    replay_tag: "REPLAY",
    total_cost: "Cost of this game",
    dead: "dead",
    survived: "alive",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_has_no_em_dash() {
        for ui in [&FR, &EN] {
            let mut all: Vec<&str> = ui.characters.iter().flat_map(|(n, p)| [*n, *p]).collect();
            all.extend([ui.subtitle, ui.keys, ui.too_small, ui.press_enter]);
            for s in all {
                assert!(!s.contains('\u{2014}'), "{s}");
            }
        }
    }

    #[test]
    fn money_format_follows_locale() {
        assert_eq!(Lang::Fr.money(1.5), "1,50 $");
        assert_eq!(Lang::En.money(1.5), "$1.50");
    }
}
