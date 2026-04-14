// Will rellocate both eventually

//TEST:
#[derive(Debug)]
pub struct ChernSettings {
    pub can_color: bool,
}

impl ChernSettings {
    pub fn new(can_color: bool) -> ChernSettings {
        ChernSettings { can_color }
    }
}

impl Default for ChernSettings {
    fn default() -> Self {
        Self {
            can_color: Default::default(),
        }
    }
}
