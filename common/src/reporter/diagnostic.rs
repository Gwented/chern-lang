pub struct Reporter {
    pub diags: Vec<Diagnostic>,
}

impl Reporter {
    pub fn new() -> Reporter {
        Reporter { diags: Vec::new() }
    }
}

// Branch...BRANCH
#[derive(Debug)]
pub enum Area {
    ConfigLoad,
    Script,
    Serial,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub msg: String,
    pub area: Area,
}

impl Diagnostic {
    pub fn new(msg: String, area: Area) -> Diagnostic {
        Diagnostic { msg, area }
    }
}
