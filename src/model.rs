use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    Haiku,
    Sonnet,
    Opus,
    Fable,
    Human,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Usage {
    pub fn add(&mut self, other: &Usage) {
        self.input += other.input;
        self.output += other.output;
        self.cache_read += other.cache_read;
        self.cache_write += other.cache_write;
    }
}

impl Model {
    pub const ALL: [Model; 4] = [Model::Haiku, Model::Sonnet, Model::Opus, Model::Fable];

    pub fn id(self) -> &'static str {
        match self {
            Model::Haiku => "claude-haiku-5-5",
            Model::Sonnet => "claude-sonnet-5-5",
            Model::Opus => "claude-opus-5-5",
            Model::Fable => "claude-fable-5-1",
            Model::Human => "human",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Model::Haiku => "Haiku 5.5",
            Model::Sonnet => "Sonnet 5.5",
            Model::Opus => "Opus 5.5",
            Model::Fable => "Fable 5.1",
            Model::Human => "Human",
        }
    }

    pub fn rgb(self) -> (u8, u8, u8) {
        match self {
            Model::Haiku => (120, 224, 172),
            Model::Sonnet => (112, 172, 255),
            Model::Opus => (196, 140, 255),
            Model::Fable => (255, 198, 92),
            Model::Human => (255, 255, 255),
        }
    }

    pub fn parse(s: &str) -> Option<Model> {
        match s.trim().to_lowercase().as_str() {
            "haiku" => Some(Model::Haiku),
            "sonnet" => Some(Model::Sonnet),
            "opus" => Some(Model::Opus),
            "fable" => Some(Model::Fable),
            _ => None,
        }
    }

    fn usd_per_million(self) -> [f64; 4] {
        match self {
            Model::Haiku => [0.10, 0.125, 0.01, 0.50],
            Model::Sonnet => [2.0, 2.50, 0.10, 10.0],
            Model::Opus => [4.0, 5.0, 0.20, 20.0],
            Model::Fable => [10.0, 12.50, 0.25, 50.0],
            Model::Human => [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn cost(self, u: &Usage) -> f64 {
        let [input, write, read, output] = self.usd_per_million();
        (u.input as f64 * input + u.cache_write as f64 * write + u.cache_read as f64 * read + u.output as f64 * output) / 1_000_000.0
    }
}

pub fn lineup(spec: &str) -> Option<Vec<Model>> {
    let spec = spec.trim().to_lowercase();
    if spec == "mixed" {
        return Some(Model::ALL.iter().flat_map(|m| [*m, *m]).collect());
    }
    if let Some(model) = Model::parse(&spec) {
        return Some(vec![model; 8]);
    }
    let models: Option<Vec<Model>> = spec.split(',').map(Model::parse).collect();
    models.filter(|m| m.len() == 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_lineup_seats_two_of_each_model() {
        let seats = lineup("mixed").unwrap();
        assert_eq!(seats.len(), 8);
        for model in Model::ALL {
            assert_eq!(seats.iter().filter(|m| **m == model).count(), 2);
        }
    }

    #[test]
    fn custom_lineup_needs_eight_seats() {
        assert!(lineup("haiku,opus").is_none());
        assert_eq!(lineup("haiku,haiku,haiku,haiku,opus,opus,fable,sonnet").unwrap().len(), 8);
        assert_eq!(lineup("opus").unwrap(), vec![Model::Opus; 8]);
    }

    #[test]
    fn cost_uses_cache_prices() {
        let usage = Usage { input: 1_000_000, output: 0, cache_read: 1_000_000, cache_write: 0 };
        assert!((Model::Opus.cost(&usage) - 4.20).abs() < 1e-9);
    }
}
