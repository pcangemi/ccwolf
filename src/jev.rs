use std::time::Duration;

use serde_json::{Map, Value, json};

use crate::game::{Judge, Role, Testimony, Verdict};

const URL: &str = "https://api.typesafe.ai/v1/systemone";
const NOBODY: &str = "nobody";
const ROLES: [(&str, Option<Role>, &str); 5] = [
    ("none", None, "The speech claims no specific role for the speaker."),
    ("villager", Some(Role::Villager), "The speaker says they are a plain villager with no power."),
    ("seer", Some(Role::Seer), "The speaker says they are the seer."),
    ("witch", Some(Role::Witch), "The speaker says they are the witch."),
    ("werewolf", Some(Role::Wolf), "The speaker admits being a werewolf."),
];

pub struct Jev {
    key: String,
    http: ureq::Agent,
}

fn role_name(role: Role) -> &'static str {
    match role {
        Role::Wolf => "werewolf",
        Role::Seer => "seer",
        Role::Witch => "witch",
        Role::Villager => "plain villager",
    }
}

impl Jev {
    pub fn from_env() -> Option<Jev> {
        let key = std::env::var("TYPESAFE_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let http = ureq::AgentBuilder::new().timeout(Duration::from_secs(15)).build();
        Some(Jev { key, http })
    }

    pub fn request(t: &Testimony) -> Value {
        let mut targets = Map::new();
        targets.insert(NOBODY.into(), json!("The speech accuses or casts suspicion on nobody in particular."));
        for name in t.names.iter().filter(|n| **n != t.speaker) {
            targets.insert(name.clone(), Value::Null);
        }
        let claims: Map<String, Value> = ROLES.iter().map(|(k, _, d)| (k.to_string(), json!(d))).collect();
        json!({
            "model": "jev-latest",
            "state": {
                "speaker": t.speaker,
                "speaker_secret_role": role_name(t.role),
                "speaker_private_knowledge": t.knows,
                "recent_public_discussion": t.recent,
                "speech": t.speech,
            },
            "questions": {
                "lie": {
                    "type": "noul",
                    "instructions": "This is a game of werewolf. Is `speech` deliberately false or misleading, given `speaker_secret_role` and `speaker_private_knowledge`? It counts as lying when a werewolf presents themself as innocent or as a villager, when anyone claims a role they do not have or denies the role they have, or when the speaker accuses a player they know to be innocent. It is sincere when the speaker states their real role, reports what they truly learned (for example a seer revealing a real vision), voices genuine suspicion, makes an honest mistake, or simply keeps their role to themself.",
                    "criteria": {
                        "true": "The speech knowingly misleads the village.",
                        "false": "The speech is sincere given what the speaker knows."
                    }
                },
                "convincing": {
                    "type": "score",
                    "instructions": "How convincing would `speech` sound to villagers who do not know `speaker_secret_role`?",
                    "criteria": [
                        "Obviously suspicious.",
                        "Rather suspicious.",
                        "Neutral.",
                        "Rather convincing.",
                        "Completely convincing."
                    ]
                },
                "claim": {
                    "type": "choice",
                    "instructions": "Which role does `speech` claim for the speaker?",
                    "criteria": claims
                },
                "accuses": {
                    "type": "choice",
                    "instructions": "Which player does `speech` mainly accuse or cast suspicion on? The speaker never accuses themself.",
                    "criteria": targets
                }
            }
        })
    }

    pub fn read(t: &Testimony, response: &Value) -> Option<Verdict> {
        let answers = &response["answers"];
        let lie = answers["lie"]["noul"].as_f64()? as f32;
        let convincing = answers["convincing"]["score"].as_f64().map(|s| s as f32);
        let claim = answers["claim"]["choice"].as_str().and_then(|c| ROLES.iter().find(|(k, _, _)| *k == c)).and_then(|(_, role, _)| *role);
        let accuses = answers["accuses"]["choice"].as_str().and_then(|name| t.names.iter().position(|n| n == name));
        Some(Verdict { lie: lie.clamp(0.0, 1.0), convincing, claim, accuses })
    }

    pub fn ask(&self, t: &Testimony) -> Result<Verdict, String> {
        let body = Jev::request(t);
        let mut wait = Duration::from_millis(300);
        for attempt in 0..3 {
            let response = self.http.post(URL).set("Authorization", &format!("Bearer {}", self.key)).send_json(body.clone());
            match response {
                Ok(r) => {
                    let value = r.into_json::<Value>().map_err(|e| format!("bad JSON from Jev: {e}"))?;
                    return Jev::read(t, &value).ok_or_else(|| "Jev answered without a verdict".to_string());
                }
                Err(ureq::Error::Status(code, _)) if [429, 500, 502, 503, 529].contains(&code) && attempt < 2 => {
                    std::thread::sleep(wait);
                    wait *= 3;
                }
                Err(ureq::Error::Status(code, r)) => {
                    let detail = r.into_string().unwrap_or_default();
                    return Err(format!("Jev error {code}: {}", detail.chars().take(200).collect::<String>()));
                }
                Err(e) if attempt < 2 => {
                    let _ = e;
                    std::thread::sleep(wait);
                    wait *= 3;
                }
                Err(e) => return Err(format!("Jev unreachable: {e}")),
            }
        }
        Err("Jev kept failing".into())
    }
}

impl Judge for Jev {
    fn judge(&self, t: &Testimony) -> Option<Verdict> {
        self.ask(t).ok()
    }
}

pub fn samples() -> Vec<(Testimony, &'static str)> {
    let names: Vec<String> = ["Marie", "Léon", "Hugo", "Basile", "Rose"].iter().map(|s| s.to_string()).collect();
    vec![
        (
            Testimony {
                speaker: "Marie".into(),
                role: Role::Wolf,
                knows: vec!["Basile est aussi loup-garou.".into(), "(Loups) La meute dévorera Rose cette nuit.".into()],
                recent: vec!["Hugo : « Marie, tu étais où cette nuit ? »".into()],
                speech: "Moi ? Simple villageoise, comme toujours. Par contre Léon me paraît bien louche depuis ce matin.".into(),
                names: names.clone(),
            },
            "wolf denying, accusing Léon: expect a lie",
        ),
        (
            Testimony {
                speaker: "Léon".into(),
                role: Role::Seer,
                knows: vec!["Ta vision : Marie est un loup-garou.".into()],
                recent: vec!["Marie : « Léon me paraît bien louche. »".into()],
                speech: "Je suis la voyante. Cette nuit, j'ai sondé Marie : c'est une louve. Votez contre elle.".into(),
                names: names.clone(),
            },
            "real seer revealing a wolf: expect the truth, claim seer, accuses Marie",
        ),
        (
            Testimony {
                speaker: "Hugo".into(),
                role: Role::Villager,
                knows: vec![],
                recent: vec![],
                speech: "Je n'ai aucune preuve, mais le silence de Basile m'inquiète un peu.".into(),
                names,
            },
            "villager with sincere doubt: expect the truth, accuses Basile",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_request_names_every_other_player_and_the_roles() {
        let (t, _) = samples().remove(0);
        let body = Jev::request(&t);
        let targets = body["questions"]["accuses"]["criteria"].as_object().unwrap();
        assert!(targets.contains_key(NOBODY));
        assert!(!targets.contains_key("Marie"));
        assert_eq!(targets.len(), t.names.len());
        assert_eq!(body["questions"]["claim"]["criteria"].as_object().unwrap().len(), 5);
        assert_eq!(body["state"]["speaker_secret_role"], "werewolf");
        assert!(!body.to_string().contains('\u{2014}'));
    }

    #[test]
    fn answers_map_back_to_a_verdict() {
        let (t, _) = samples().remove(1);
        let response = json!({
            "answers": {
                "lie": { "type": "noul", "noul": 0.08 },
                "convincing": { "type": "score", "score": 4.2 },
                "claim": { "type": "choice", "choice": "seer" },
                "accuses": { "type": "choice", "choice": "Marie" }
            }
        });
        let v = Jev::read(&t, &response).unwrap();
        assert!((v.lie - 0.08).abs() < 1e-6);
        assert_eq!(v.claim, Some(Role::Seer));
        assert_eq!(v.accuses, Some(0));
        let none = json!({ "answers": { "lie": { "noul": 0.5 }, "claim": { "choice": "none" }, "accuses": { "choice": "nobody" } } });
        let v = Jev::read(&t, &none).unwrap();
        assert_eq!((v.claim, v.accuses, v.convincing), (None, None, None));
    }
}
