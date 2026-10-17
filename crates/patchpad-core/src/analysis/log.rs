//! Journal des événements (appuis, relâchements, connexions).

use std::collections::VecDeque;

use serde::Serialize;

use crate::model::{Button, Buttons};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogKind {
    Pressed,
    Released,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    /// Numéro croissant, pour que le frontend n'ajoute que les nouvelles entrées.
    pub seq: u64,
    /// Millisecondes depuis le démarrage de l'analyse.
    pub t_ms: u64,
    pub controller_id: String,
    pub controller_name: String,
    pub kind: LogKind,
    pub button: Option<Button>,
}

#[derive(Debug, Clone)]
pub struct EventLog {
    entries: VecDeque<LogEntry>,
    capacity: usize,
    next_seq: u64,
}

impl EventLog {
    pub fn new(capacity: usize) -> Self {
        EventLog { entries: VecDeque::new(), capacity, next_seq: 1 }
    }

    pub fn push(
        &mut self,
        t_ms: u64,
        controller_id: &str,
        controller_name: &str,
        kind: LogKind,
        button: Option<Button>,
    ) {
        let entry = LogEntry {
            seq: self.next_seq,
            t_ms,
            controller_id: controller_id.to_owned(),
            controller_name: controller_name.to_owned(),
            kind,
            button,
        };
        self.next_seq += 1;
        self.entries.push_back(entry);
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }

    /// Ajoute les appuis et relâchements entre deux états de boutons.
    pub fn push_diff(
        &mut self,
        t_ms: u64,
        controller_id: &str,
        controller_name: &str,
        before: Buttons,
        after: Buttons,
    ) {
        for button in Button::ALL {
            match (before.contains(button), after.contains(button)) {
                (false, true) => self.push(t_ms, controller_id, controller_name, LogKind::Pressed, Some(button)),
                (true, false) => self.push(t_ms, controller_id, controller_name, LogKind::Released, Some(button)),
                _ => {}
            }
        }
    }

    /// Entrées de numéro strictement supérieur à `seq`.
    pub fn since(&self, seq: u64) -> Vec<LogEntry> {
        self.entries.iter().filter(|e| e.seq > seq).cloned().collect()
    }

    pub fn last_seq(&self) -> u64 {
        self.next_seq - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_produces_press_and_release() {
        let mut log = EventLog::new(10);
        let before: Buttons = [Button::South, Button::L1].into_iter().collect();
        let after: Buttons = [Button::L1, Button::Home].into_iter().collect();
        log.push_diff(5, "a", "A", before, after);
        let entries = log.since(0);
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[0].kind, entries[0].button), (LogKind::Released, Some(Button::South)));
        assert_eq!((entries[1].kind, entries[1].button), (LogKind::Pressed, Some(Button::Home)));
    }

    #[test]
    fn capacity_and_since() {
        let mut log = EventLog::new(3);
        for t in 0..5 {
            log.push(t, "a", "A", LogKind::Connected, None);
        }
        assert_eq!(log.last_seq(), 5);
        let all = log.since(0);
        assert_eq!(all.iter().map(|e| e.seq).collect::<Vec<_>>(), vec![3, 4, 5]);
        assert_eq!(log.since(4).len(), 1);
        assert!(log.since(5).is_empty());
    }
}
