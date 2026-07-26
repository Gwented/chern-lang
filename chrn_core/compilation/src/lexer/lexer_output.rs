use crate::lexer::{token::SpannedToken, trivia::Trivia};

/// Container for lexer-specific output data
pub struct LexerOutput {
    /// Bloop bloop
    pub toks: Vec<SpannedToken>,
    /// Bloop bloop bolbobp
    pub trivia: Vec<Trivia>,
    /// Amount of `Token::Invalid` spotted
    pub found_invalid_toks: u8,
}

impl LexerOutput {
    //Yes.
    pub fn new(toks: Vec<SpannedToken>, trivia: Vec<Trivia>, found_invalid_toks: u8) -> Self {
        Self {
            toks,
            trivia,
            found_invalid_toks,
        }
    }
}
