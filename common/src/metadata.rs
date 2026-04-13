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

// Should not have can color
#[derive(Debug)]
pub struct ModuleMetadata {
    // Should maybe be path id
    /// Bytes from chern config file
    pub src_bytes: Vec<u8>,
    // / Amount of \n within config file so binary search can be done by error reporter
    // pub new_lines: Vec<usize>,
    /// The script language start which can be different depending on if @def is used
    pub script_start: usize,
    /// The serial start which can be None if there is no serialized file within the config file
    pub serial_start: Option<usize>,
}

impl ModuleMetadata {
    pub fn new(
        src_bytes: Vec<u8>,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleMetadata {
        ModuleMetadata {
            // new_lines: Vec::new(),
            src_bytes,
            script_start,
            serial_start,
            //TODO: Could be env var
        }
    }
}
