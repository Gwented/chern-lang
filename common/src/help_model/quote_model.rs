mod lexer;

use std::ops::Range;

use crate::{
    help_model::algo,
    keywords::{self, DEFINITION_SIZE},
    symbols::Span,
};

pub enum State {
    SearchingForQuotes,
    InQuotes,
    // In def
}

const W_TOK: f32 = 0.6;
const W_POS: f32 = 1.0;

/// Basic state of not needing any special actions
const PROCEED: u8 = 0;
/// Action to cut portion of array
const CUT: u8 = 1;

#[derive(Debug)]
pub struct QuoteGraph {
    current_idx: usize,
    q_nodes: Vec<QuoteNode>,
    weights: [f32; 4],
}

impl QuoteGraph {
    fn new() -> QuoteGraph {
        // alphanum, \n, /, everything else, @def
        let weights = [0.20, 0.10, 0.6, 0.10];

        QuoteGraph {
            current_idx: 0,
            q_nodes: Vec::new(),
            weights,
        }
    }

    /// Creates a new node and sets it as it's current to be adjusted
    fn next_node(&mut self, start_pos: usize) {
        self.current_idx = self.q_nodes.len();

        let q_node = QuoteNode::new(start_pos);
        self.q_nodes.push(q_node);
    }

    fn adjust_node(&mut self, current_ch: char) {
        let q_node = &self.q_nodes[self.current_idx];
        let steps = (q_node.ctx_toks.len() + 1) as f32;

        let start_pos = q_node.start_pos as f32;

        // Weight of tokens overall
        // Weight of positioning

        let tok_sig = self.weights[translate_tok(current_ch)];

        // Exponential decrease of positional bias as the starting position increases
        let pos_sig = (-(2.0f32.ln()) * (start_pos)).exp();

        let q_node = &mut self.q_nodes[self.current_idx];

        q_node.score += ((W_TOK * tok_sig) + (W_POS * pos_sig)) * q_node.rate;

        q_node.ctx_toks.push(current_ch);
    }

    // This is compensation for there being no built context window
    /// Evaluates a singular node
    fn eval(&mut self) {
        let q_node = &mut self.q_nodes[self.current_idx];

        let end_pos = q_node.start_pos + q_node.ctx_toks.len() + 1;
        q_node.end_pos = Some(end_pos);

        let mut i = 0;

        let w_def = 1.5;
        let w_end = 1.5;

        while i < q_node.ctx_toks.len() {
            let ch = q_node.ctx_toks[i];

            if (ch == '@' && i + DEFINITION_SIZE < q_node.ctx_toks.len())
                && q_node.ctx_toks[i..i + DEFINITION_SIZE] == ['@', 'd', 'e', 'f']
            {
                q_node.concern /= q_node.score * w_def;
                i += DEFINITION_SIZE;
            } else if (ch == '@' && i + DEFINITION_SIZE < q_node.ctx_toks.len())
                && q_node.ctx_toks[i..i + DEFINITION_SIZE] == ['@', 'e', 'n', 'd']
            {
                q_node.concern /= q_node.score * w_end;
                i += DEFINITION_SIZE;
            }

            i += 1;
        }
    }

    /// Evaluates all nodes
    // Suspicious name
    fn final_eval(&mut self) {
        let score_logits: Vec<f32> = self.q_nodes.iter().map(|n| n.score).collect();

        let max_score_idx = algo::argmax(&score_logits);

        let concern_logits: Vec<f32> = self.q_nodes.iter().map(|n| n.concern).collect();

        let max_concern_idx = algo::argmax(&concern_logits);

        let action =
            self.choose_action(score_logits[max_score_idx], concern_logits[max_concern_idx]);

        //NOTE: This is always safe due to q_node needing to be at least 2 for quote probs to start
        if action == CUT && max_concern_idx + 1 < self.q_nodes.len() {
            self.q_nodes.truncate(max_concern_idx + 1);
        }
        dbg!(&self.q_nodes);
    }

    // Proceeds with operations normally or cuts off quotes to basically say, the rest of the
    // probabilities are likely not to be the end quote

    //WARN: Currently does not use this correctly
    fn choose_action(&self, proceed: f32, mut cut: f32) -> u8 {
        cut = cut / proceed;

        if cut > 0.6 { CUT } else { PROCEED }
    }
}

#[derive(Debug)]
struct QuoteNode {
    start_pos: usize,
    end_pos: Option<usize>,
    score: f32,
    rate: f32,
    concern: f32,
    ctx_toks: Vec<char>,
}

// Need variable proportional to the actual file size
impl QuoteNode {
    fn new(start_pos: usize) -> QuoteNode {
        QuoteNode {
            start_pos,
            end_pos: None,
            score: 0.05,
            rate: 0.005,
            concern: 0.05,
            ctx_toks: Vec::new(),
        }
    }
}

fn translate_tok(ch: char) -> usize {
    match ch {
        c if c.is_alphanumeric() => 0,
        '\n' => 1,
        '/' => 2,
        _ => 3,
    }
}

// ITS RECOVERABLE. IT CAN RECOVER, IT CAN DO IT. IT DOESN'T NEED REAL TOKENS
/// Predicts where an unclosed quote may have started
pub fn quote_start_probability(src: &[u8], q_type: char, search_range: Range<usize>) -> Vec<Span> {
    let mut q_graph = QuoteGraph::new();

    let mut state = State::SearchingForQuotes;

    // How many characters were seen before the next quote
    // WARN: Not handled cleanly as of right now
    let src_str = str::from_utf8(src).expect("Invalid UTF-8 within file");

    for (i, ch) in src_str.chars().enumerate() {
        match state {
            State::SearchingForQuotes => {
                if ch == q_type {
                    q_graph.next_node(i);
                    state = State::InQuotes;
                }
            }
            State::InQuotes => match ch {
                c if c == q_type => {
                    q_graph.eval();
                    state = State::SearchingForQuotes;
                }
                c => {
                    q_graph.adjust_node(c);
                }
            },
        }
    }

    q_graph.final_eval();

    let mut res_idx: usize = 0;
    let mut largest_score = 0.0;

    for (i, score) in q_graph.q_nodes.iter().map(|q| q.score).enumerate() {
        if score > largest_score {
            largest_score = score;
            res_idx = i;
        }
    }

    let highest_q = &q_graph.q_nodes[res_idx];

    let start = highest_q.start_pos + search_range.start;
    let end = highest_q.end_pos.unwrap_or(highest_q.start_pos) + search_range.start;

    //FIX:
    let first = Span::new(start, start);
    let last = Span::new(end, end);

    vec![first, last]
}
