// Neutral states
const DEFAULT_FLAG: u16 = 1 << 0;
const BIND_FLAG: u16 = 1 << 1;
const ALIAS_FLAG: u16 = 1 << 2;
// Sections
const VAR_FLAG: u16 = 1 << 3;
const NEST_FLAG: u16 = 1 << 4;
const COMPLEX_FLAG: u16 = 1 << 5;
const OVERRIDE_FLAG: u16 = 1 << 6;

const NEUTRAL_SET: u16 = DEFAULT_FLAG | BIND_FLAG | ALIAS_FLAG;

pub(super) struct ParserState {
    pub(super) flag: u16,
}

// Could reduce duplication by manually enforcing setting it from the outside but im scared
impl ParserState {
    pub(super) fn new() -> ParserState {
        ParserState { flag: DEFAULT_FLAG }
    }

    //WARN: Not sure if this is needed since sections already enforce only sections are next
    // It actually doesn't matter if it's neutral or not besides visually from what I can tell
    // May remove.
    pub(super) fn is_neutral(&self) -> bool {
        // If self.flag's value exists within neutral set
        (self.flag | NEUTRAL_SET) == NEUTRAL_SET
    }

    // Is this elixer?
    pub(super) fn flip_bind(&mut self) {
        self.flag |= BIND_FLAG;
    }

    pub(super) fn has_bind(&self) -> bool {
        // If the corresponding bits are != 0 then the bit is not in the set so has_bind == false
        (self.flag & BIND_FLAG) != 0
    }

    pub(super) fn flip_alias(&mut self) {
        self.flag |= ALIAS_FLAG;
    }

    pub(super) fn has_alias(&self) -> bool {
        (self.flag & ALIAS_FLAG) != 0
    }

    pub(super) fn flip_var(&mut self) {
        self.flag |= VAR_FLAG;
    }

    pub(super) fn has_var(&self) -> bool {
        (self.flag & VAR_FLAG) != 0
    }

    pub(super) fn flip_nest(&mut self) {
        self.flag |= NEST_FLAG;
    }

    pub(super) fn has_nest(&self) -> bool {
        (self.flag & NEST_FLAG) != 0
    }

    pub(super) fn flip_complex(&mut self) {
        self.flag |= COMPLEX_FLAG;
    }

    pub(super) fn has_complex(&self) -> bool {
        (self.flag & COMPLEX_FLAG) != 0
    }

    pub(super) fn flip_override(&mut self) {
        self.flag |= OVERRIDE_FLAG;
    }

    pub(super) fn has_override(&self) -> bool {
        (self.flag & OVERRIDE_FLAG) != 0
    }
}
