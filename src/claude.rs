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
    tools: Value,
    messages: Vec<Value>,
    pending_tool: Option<String>,
}

impl LlmAgent {
    pub fn new(claude: Arc<Claude>, me: usize, lang: Lang, players: &[PlayerInfo]) -> Self {
        Self {
            claude,
            model: players[me].model,
            lang,
            players: players.to_vec(),
            system: prompts::system(lang, me, players),
            tools: prompts::tools(lang, players),
            messages: Vec::new(),
            pending_tool: None,
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
        json!({
            "model": self.model.id(),
            "max_tokens": max_tokens,
            "system": [{ "type": "text", "text": self.system, "cache_control": { "type": "ephemeral" } }],
            "tools": self.tools,
            "tool_choice": { "type": "tool", "name": prompts::TOOL, "disable_parallel_tool_use": true },
            "messages": messages,
        })
    }

    fn parse(&self, task: &Task, input: &Value) -> Option<Action> {
        let text = |key: &str| input[key].as_str().unwrap_or_default().trim().to_string();
        let thought = text("thought");
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

fn usage_of(response: &Value) -> Usage {
    let n = |k: &str| response["usage"][k].as_u64().unwrap_or(0);
    Usage { input: n("input_tokens"), output: n("output_tokens"), cache_read: n("cache_read_input_tokens"), cache_write: n("cache_creation_input_tokens") }
}

impl Agent for LlmAgent {
    fn act(&mut self, news: &[String], task: &Task) -> Result<Reply, String> {
        let mut content = Vec::new();
        if let Some(id) = self.pending_tool.take() {
            content.push(json!({ "type": "tool_result", "tool_use_id": id, "content": "ok" }));
        }
        let mut text = news.join("\n");
        if !text.is_empty() {
            text.push_str("\n\n");
        }
        text.push_str(&prompts::task_text(self.lang, task, &self.players));
        content.push(json!({ "type": "text", "text": text }));
        self.messages.push(json!({ "role": "user", "content": content }));

        let mut usage = Usage::default();
        for max_tokens in [700, 1600, 1600] {
            let response = self.claude.message(&self.request(max_tokens))?;
            usage.add(&usage_of(&response));
            let truncated = response["stop_reason"].as_str() == Some("max_tokens");
            let call = response["content"].as_array().and_then(|blocks| blocks.iter().find(|b| b["type"] == "tool_use")).cloned();
            let Some(call) = call.filter(|_| !truncated) else { continue };
            let Some(action) = self.parse(task, &call["input"]) else { continue };
            self.pending_tool = call["id"].as_str().map(String::from);
            self.messages.push(json!({ "role": "assistant", "content": [call] }));
            return Ok(Reply { action: Some(action), usage });
        }
        self.messages.pop();
        Err(format!("{} gave no usable answer", self.model.label()))
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
        LlmAgent::new(claude, 0, Lang::Fr, &players)
    }

    #[test]
    fn only_the_last_block_carries_a_cache_breakpoint() {
        let mut a = agent();
        a.messages.push(json!({ "role": "user", "content": [{ "type": "text", "text": "a", "cache_control": { "type": "ephemeral" } }] }));
        a.messages.push(json!({ "role": "assistant", "content": [{ "type": "tool_use", "id": "x", "name": "speak", "input": {} }] }));
        a.messages
            .push(json!({ "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "x", "content": "ok" }, { "type": "text", "text": "b" }] }));
        let body = a.request(700);
        let marks = body["messages"].to_string().matches("cache_control").count();
        assert_eq!(marks, 1);
        assert!(body["messages"][2]["content"][1]["cache_control"].is_object());
        assert_eq!(body["tool_choice"]["name"], prompts::TOOL);
    }

    #[test]
    fn names_map_back_to_seats_and_unknown_names_do_not() {
        let a = agent();
        let name = a.players[3].name.to_uppercase();
        assert_eq!(a.player(&name), Some(3));
        assert_eq!(a.player("personne"), None);
        let action = a.parse(
            &Task::Witch { day: 1, victim: None, can_save: false, can_poison: true, targets: vec![] },
            &json!({ "thought": "t", "save": false, "poison": "personne" }),
        );
        assert_eq!(action, Some(Action::Potion { thought: "t".into(), save: false, poison: None }));
    }

    #[test]
    fn an_answer_missing_the_asked_field_is_rejected_so_it_can_be_retried() {
        let a = agent();
        assert_eq!(a.parse(&Task::Speak { day: 1, round: 1 }, &json!({ "thought": "t" })), None);
        assert_eq!(a.parse(&Task::Vote { day: 1, targets: vec![1] }, &json!({ "thought": "t", "target": "Nobody At All" })), None);
        let name = a.players[2].name.clone();
        assert_eq!(
            a.parse(&Task::Vote { day: 1, targets: vec![2] }, &json!({ "thought": "t", "target": name })),
            Some(Action::Choose { thought: "t".into(), target: Some(2) })
        );
    }
}
