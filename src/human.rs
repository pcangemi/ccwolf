use std::sync::mpsc::{Receiver, Sender, channel};

use crate::game::{Action, Agent, Reply, Task};

pub struct Request {
    pub task: Task,
}

pub struct HumanAgent {
    requests: Sender<Request>,
    answers: Receiver<Action>,
}

pub struct Seat {
    pub seat: usize,
    pub requests: Receiver<Request>,
    pub answers: Sender<Action>,
}

pub fn pair(seat: usize) -> (HumanAgent, Seat) {
    let (request_tx, request_rx) = channel();
    let (answer_tx, answer_rx) = channel();
    (HumanAgent { requests: request_tx, answers: answer_rx }, Seat { seat, requests: request_rx, answers: answer_tx })
}

impl Agent for HumanAgent {
    fn act(&mut self, _news: &[String], task: &Task) -> Result<Reply, String> {
        self.requests.send(Request { task: task.clone() }).map_err(|_| "the player left the game".to_string())?;
        let action = self.answers.recv().map_err(|_| "the player left the game".to_string())?;
        Ok(Reply { action: Some(action), usage: Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_agent_waits_for_the_screen_to_answer() {
        let (mut agent, seat) = pair(3);
        let screen = std::thread::spawn(move || {
            let request = seat.requests.recv().unwrap();
            assert!(matches!(request.task, Task::Vote { .. }));
            seat.answers.send(Action::Choose { thought: String::new(), target: Some(5) }).unwrap();
        });
        let reply = agent.act(&[], &Task::Vote { day: 1, targets: vec![5, 6] }).unwrap();
        assert_eq!(reply.action, Some(Action::Choose { thought: String::new(), target: Some(5) }));
        screen.join().unwrap();
    }
}
