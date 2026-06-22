use edgequake_llm::ChatMessage;

pub struct Session {
    pub id: usize,
    pub name: String,
    pub provider_name: String,
    pub model: String,
    pub provider: Option<crate::ProviderBox>,
    pub messages: Vec<ChatMessage>,
}

pub struct SessionManager {
    pub sessions: Vec<Session>,
    pub active_idx: Option<usize>,
    next_id: usize,
}

impl SessionManager {
    pub fn new() -> Self {
        Self { sessions: vec![], active_idx: None, next_id: 1 }
    }

    pub fn add(&mut self, name: String, provider_name: String, model: String, provider: crate::ProviderBox) {
        let id = self.next_id;
        self.next_id += 1;
        self.sessions.push(Session {
            id, name, provider_name, model, provider: Some(provider), messages: vec![],
        });
        self.active_idx = Some(self.sessions.len() - 1);
    }

    pub fn active_mut(&mut self) -> Option<&mut Session> {
        self.active_idx.map(|i| &mut self.sessions[i])
    }

    pub fn active(&self) -> Option<&Session> {
        self.active_idx.map(|i| &self.sessions[i])
    }

    pub fn switch(&mut self, id: usize) -> bool {
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            self.active_idx = Some(pos);
            true
        } else { false }
    }

    pub fn remove(&mut self, id: usize) -> bool {
        if let Some(pos) = self.sessions.iter().position(|s| s.id == id) {
            self.sessions.remove(pos);
            if self.sessions.is_empty() {
                self.active_idx = None;
            } else if self.active_idx == Some(pos) {
                self.active_idx = Some(if pos > 0 { pos - 1 } else { 0 });
            }
            true
        } else { false }
    }

    pub fn list(&self) {
        if self.sessions.is_empty() {
            println!("No active sessions.");
            return;
        }
        for (i, s) in self.sessions.iter().enumerate() {
            let active = self.active_idx == Some(i);
            println!("  {}{}. [{}] {} / {} ({} msgs)",
                if active { ">" } else { " " },
                s.id, s.name, s.provider_name, s.model, s.messages.len());
        }
    }
}
