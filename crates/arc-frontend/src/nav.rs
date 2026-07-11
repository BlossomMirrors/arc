#[derive(Clone, Debug, Default)]
pub struct NavEntry {
    pub view: String,
    pub detail_id: String,
    #[allow(dead_code)]
    pub search_text: String,
}

pub struct NavHistory {
    entries: Vec<NavEntry>,
    cursor: usize,
}

impl NavHistory {
    pub fn new() -> Self {
        Self {
            entries: vec![NavEntry {
                view: "home".to_string(),
                ..Default::default()
            }],
            cursor: 0,
        }
    }

    pub fn push(&mut self, entry: NavEntry) {
        self.entries.truncate(self.cursor + 1);
        self.entries.push(entry);
        self.cursor = self.entries.len() - 1;
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor < self.entries.len() - 1
    }

    pub fn back(&mut self) -> Option<NavEntry> {
        if self.cursor > 0 {
            self.cursor -= 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    pub fn forward(&mut self) -> Option<NavEntry> {
        if self.cursor < self.entries.len() - 1 {
            self.cursor += 1;
            Some(self.entries[self.cursor].clone())
        } else {
            None
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
