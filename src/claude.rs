use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use crate::game::{Action, Agent, PlayerInfo, Reply, Task};
use crate::lang::Lang;
use crate::model::{Model, Usage};
use crate::prompts;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const RETRYABLE: [u16; 5] = [429, 500, 502, 503, 529];

pub struct Claude {
    key: String,
    http: ureq::Agent,
}

impl Claude {
    pub fn from_env() -> Option<Arc<Claude>> {
        let key = std::env::var("ANTHROPIC_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let http = ureq::AgentBuilder::new().timeout(Duration::from_secs(120)).build();
        Some(Arc::new(Claude { key, http }))
    }

    pub fn message(&self, body: &Value) -> Result<Value, String> {
        let mut wait = Duration::from_secs(2);
        for attempt in 0..6 {
            let response = self.http.post(API_URL).set("x-api-key", &self.key).set("anthropic-version", "2023-06-01").send_json(body.clone());
            match response {
                Ok(r) => {
                    return r.into_json::<Value>().map_err(|e| format!("bad JSON from the API: {e}"));
                }
                Err(ureq::Error::Status(code, r)) if RETRYABLE.contains(&code) && attempt < 5 => {
                    let hinted = r.header("retry-after").and_then(|s| s.parse::<u64>().ok()).map(Duration::from_secs);
                    std::thread::sleep(hinted.unwrap_or(wait).min(Duration::from_secs(60)));
                    wait *= 2;
                }
                Err(ureq::Error::Status(code, r)) => {
                    let detail = r.into_json::<Value>().ok().and_then(|v| v["error"]["message"].as_str().map(String::from)).unwrap_or_default();
                    return Err(format!("API error {code}: {detail}"));
                }
                Err(e) if attempt < 5 => {
                    let _ = e;
                    std::thread::sleep(wait);
                    wait *= 2;
                }
                Err(e) => return Err(format!("network error: {e}")),
            }
        }
        Err("the API kept failing".into())
    }
}

pub struct LlmAgent {
    claude: Arc<Claude>,
    model: Model,
    lang: Lang,
    players: Vec<PlayerInfo>,
    system: String,
    schema: Value,
    effort: Option<String>,
    messages: Vec<Value>,
}

impl LlmAgent {
    pub fn new(claude: Arc<Claude>, me: usize, lang: Lang, players: &[PlayerInfo], effort: Option<String>) -> Self {
        Self {
            claude,
            model: players[me].model,
            lang,
            players: players.to_vec(),
            system: prompts::system(lang, me, players),
            schema: prompts::schema(lang, players),
            effort,
            messages: Vec::new(),
        }
    }

    fn player(&self, name: &str) -> Option<usize> {
        let wanted = name.trim().to_lowercase();
        self.players.iter().position(|p| p.name.to_lowercase() == wanted)
    }

    fn request(&self, max_tokens: u32) -> Value {
        let mut messages = self.messages.clone();
        for message in messages.iter_mut() {
            if let Some(blocks) = message["content"].as_array_mut() {
                for block in blocks.iter_mut() {
                    if let Some(map) = block.as_object_mut() {
                        map.remove("cache_control");
                    }
                }
            }
        }
        if let Some(last) = messages.last_mut().and_then(|m| m["content"].as_array_mut()).and_then(|b| b.last_mut()) {
            last["cache_control"] = json!({ "type": "ephemeral" });
        }
        let mut output = json!({ "format": { "type": "json_schema", "schema": self.schema } });
        if let Some(effort) = &self.effort {
            output["effort"] = json!(effort);
        }
        json!({
            "model": self.model.id(),
            "max_tokens": max_tokens,
            "system": [{ "type": "text", "text": self.system, "cache_control": { "type": "ephemeral" } }],
            "output_config": output,
            "messages": messages,
        })
    }

    fn parse(&self, task: &Task, input: &Value) -> Option<Action> {
        let text = |key: &str| without_dashes(input[key].as_str().unwrap_or_default());
        let thought = text("aside");
        match task {
            Task::WolfTalk { .. } | Task::Speak { .. } | Task::LastWords => {
                let say = text("speech");
                (!say.is_empty()).then_some(Action::Speak { thought, say })
            }
            Task::WolfPick { .. } | Task::Seer { .. } | Task::Vote { .. } => {
                self.player(&text("target")).map(|target| Action::Choose { thought, target: Some(target) })
            }
            Task::Witch { .. } => Some(Action::Potion { thought, save: input["save"].as_bool().unwrap_or(false), poison: self.player(&text("poison")) }),
        }
    }
}

fn without_dashes(text: &str) -> String {
    text.replace(" \u{2014} ", ", ").replace("\u{2014} ", ", ").replace(" \u{2014}", ",").replace('\u{2014}', ", ").trim().to_string()
}

fn usage_of(response: &Value) -> Usage {
    let n = |k: &str| response["usage"][k].as_u64().unwrap_or(0);
    Usage { input: n("input_tokens"), output: n("output_tokens"), cache_read: n("cache_read_input_tokens"), cache_write: n("cache_creation_input_tokens") }
}

fn answer_text(response: &Value) -> Option<String> {
    let blocks = response["content"].as_array()?;
    let text: String = blocks.iter().filter(|b| b["type"] == "text").filter_map(|b| b["text"].as_str()).collect();
    (!text.trim().is_empty()).then_some(text)
}

impl Agent for LlmAgent {
    fn act(&mut self, news: &[String], task: &Task) -> Result<Reply, String> {
        let mut text = news.join("\n");
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&prompts::task_text(self.lang, task, &self.players));
        self.messages.push(json!({ "role": "user", "content": [{ "type": "text", "text": text }] }));

        let mut usage = Usage::default();
        for max_tokens in [6000, 12000, 12000] {
            let response = self.claude.message(&self.request(max_tokens))?;
            usage.add(&usage_of(&response));
            if response["stop_reason"].as_str() == Some("max_tokens") {
                continue;
            }
            let Some(answer) = answer_text(&response) else { continue };
            let Ok(input) = serde_json::from_str::<Value>(&answer) else { continue };
            let Some(action) = self.parse(task, &input) else { continue };
            self.messages.push(json!({ "role": "assistant", "content": [{ "type": "text", "text": answer }] }));
            return Ok(Reply { action: Some(action), usage });
        }
        self.messages.push(json!({ "role": "assistant", "content": [{ "type": "text", "text": json!({ "aside": "..." }).to_string() }] }));
        Ok(Reply { action: None, usage })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::deal;
    use crate::model::lineup;

    fn agent() -> LlmAgent {
        let players = deal(5, Lang::Fr, &lineup("mixed").unwrap());
        let claude = Arc::new(Claude { key: "test".into(), http: ureq::Agent::new() });
        LlmAgent::new(claude, 0, Lang::Fr, &players, None)
    }

    #[test]
    fn only_the_last_block_carries_a_cache_breakpoint() {
        let mut a = agent();
        a.messages.push(json!({ "role": "user", "content": [{ "type": "text", "text": "a", "cache_control": { "type": "ephemeral" } }] }));
        a.messages.push(json!({ "role": "assistant", "content": [{ "type": "text", "text": "{\"aside\":\"x\"}" }] }));
        a.messages.push(json!({ "role": "user", "content": [{ "type": "text", "text": "b" }] }));
        let body = a.request(6000);
        let marks = body["messages"].to_string().matches("cache_control").count();
        assert_eq!(marks, 1);
        assert!(body["messages"][2]["content"][0]["cache_control"].is_object());
        assert_eq!(body["output_config"]["format"]["type"], "json_schema");
        assert!(body.get("tools").is_none() && body.get("tool_choice").is_none(), "Sonnet, Opus and Fable 5.x reject a forced tool");
        assert!(body["output_config"].get("effort").is_none());
    }

    #[test]
    fn em_dashes_from_the_models_become_commas() {
        assert_eq!(without_dashes("Rose gone \u{2014} poisoned, surely."), "Rose gone, poisoned, surely.");
        assert_eq!(without_dashes("a\u{2014}b"), "a, b");
        let a = agent();
        let action = a.parse(&Task::Speak { day: 1, round: 1 }, &json!({ "aside": "x \u{2014} y", "speech": "Bien \u{2014} merci" }));
        assert_eq!(action, Some(Action::Speak { thought: "x, y".into(), say: "Bien, merci".into() }));
    }

    #[test]
    fn the_schema_is_the_same_for_every_task_so_the_cache_holds() {
        let a = agent();
        assert_eq!(a.request(6000)["output_config"], a.request(12000)["output_config"]);
    }

    #[test]
    fn names_map_back_to_seats_and_unknown_names_do_not() {
        let a = agent();
        let name = a.players[3].name.to_uppercase();
        assert_eq!(a.player(&name), Some(3));
        assert_eq!(a.player("personne"), None);
        let action = a.parse(
            &Task::Witch { day: 1, victim: None, can_save: false, can_poison: true, targets: vec![] },
            &json!({ "aside": "t", "save": false, "poison": "personne" }),
        );
        assert_eq!(action, Some(Action::Potion { thought: "t".into(), save: false, poison: None }));
    }

    #[test]
    fn an_answer_missing_the_asked_field_is_rejected_so_it_can_be_retried() {
        let a = agent();
        assert_eq!(a.parse(&Task::Speak { day: 1, round: 1 }, &json!({ "aside": "t" })), None);
        assert_eq!(a.parse(&Task::Vote { day: 1, targets: vec![1] }, &json!({ "aside": "t", "target": "Nobody At All" })), None);
        let name = a.players[2].name.clone();
        assert_eq!(
            a.parse(&Task::Vote { day: 1, targets: vec![2] }, &json!({ "aside": "t", "target": name })),
            Some(Action::Choose { thought: "t".into(), target: Some(2) })
        );
    }

    #[test]
    #[ignore = "calls the Anthropic API: cargo test live_probe -- --ignored --nocapture"]
    fn live_probe_every_model_answers_in_character() {
        let claude = Claude::from_env().expect("ANTHROPIC_API_KEY");
        for lang in [Lang::Fr, Lang::En] {
            let mut players = deal(5, lang, &lineup("mixed").unwrap());
            for model in Model::ALL {
                for seat in players.iter_mut() {
                    seat.model = model;
                }
                let wolf = players.iter().position(|p| p.role == crate::game::Role::Wolf).unwrap();
                let mut agent = LlmAgent::new(claude.clone(), wolf, lang, &players, None);
                let news = vec![prompts::describe(lang, &crate::game::Event::Dawn { day: 1, deaths: vec![(wolf + 3) % 8] }, wolf, &players).unwrap()];
                let reply = agent.act(&news, &Task::Speak { day: 1, round: 1 }).expect("API call");
                let cost = model.cost(&reply.usage);
                println!("{:?} {:<10} {:?} out={} cost=${cost:.4}", lang, model.label(), reply.action, reply.usage.output);
                assert!(matches!(reply.action, Some(Action::Speak { .. })), "{} gave no speech", model.label());
            }
        }
    }
}
