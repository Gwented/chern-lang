//TEST:
#[derive(Debug)]
pub struct ChrnSettings {
    pub can_color: bool,
}

impl ChrnSettings {
    pub fn new(can_color: bool) -> ChrnSettings {
        ChrnSettings { can_color }
    }
}

impl Default for ChrnSettings {
    fn default() -> Self {
        Self {
            can_color: Default::default(),
        }
    }
}
