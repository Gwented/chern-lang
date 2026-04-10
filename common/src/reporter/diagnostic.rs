pub struct Reporter {
    pub diags: Vec<Diagnostic>,
}

impl Reporter {
    pub fn new() -> Reporter {
        Reporter { diags: Vec::new() }
    }
}

// Branch...BRANCH
pub enum Area {
    ConfigLoad,
    Script,
    Serial,
}

pub struct Diagnostic {
    msg: String,
    area: Area,
}
