//TODO: Should be able to truncate current pos + 1 (cut) and reposition self given something like
//@end twice, meaning it should probably go to the node at the first @end

// None of this is supposed to be efficient since 1, this will rarely ever happen, and 2, learning.
mod lexer;
mod token;

use std::{cmp, ops::Range};

use crate::{
    help_model::{
        algo,
        quote_model::{
            self,
            token::{Token, TokenInfo},
        },
    },
    symbols::Span,
};

const W_TOK: f32 = 0.6;
const W_POS: f32 = 1.0;
const W_STEP: f32 = 3.5;

/// Basic state of not needing any special actions
const PROCEED: u8 = 0;
/// Action to cut portion of array
const CUT: u8 = 1;

#[derive(Debug)]
pub struct QuoteGraph<'a> {
    current_idx: usize,
    q_nodes: Vec<QuoteNode>,
    ctx_toks: &'a Vec<TokenInfo>,
    end_tok_pos: Option<usize>,
}

impl QuoteGraph<'_> {
    fn new(ctx_toks: &Vec<TokenInfo>) -> QuoteGraph<'_> {
        QuoteGraph {
            current_idx: 0,
            q_nodes: Vec::new(),
            ctx_toks,
            end_tok_pos: None,
        }
    }

    fn display_scores(&self) {
        for (i, q_node) in self.q_nodes.iter().enumerate() {
            println!(
                "Node {i}: \nstart: {} | end: {:?}\nscore: {} ",
                q_node.start_pos, q_node.end_pos, q_node.score
            );
        }
    }

    /// Creates a new node and sets it as it's current to be adjusted
    fn next_node(&mut self, start_pos: usize) {
        self.current_idx = self.q_nodes.len();

        let q_node = QuoteNode::new(start_pos);
        self.q_nodes.push(q_node);
    }

    fn adjust_node(&mut self, tok_info: TokenInfo) {
        let q_node = &self.q_nodes[self.current_idx];

        let start_pos = q_node.start_pos as f32;

        let distance = (q_node.ctx_toks.len() + 1) as f32;

        if tok_info.tok == Token::End {
            self.end_tok_pos = Some(self.current_idx);
        }

        let tok_sig = tok_info.sig * context_tok(&q_node.ctx_toks, &tok_info);

        // Exponential decrease of positional bias as the starting position increases
        let pos_sig = (-(2.0f32.ln()) * (start_pos + 0.20)).exp();

        // Extreme bias towards being at the beginning to catch """ more accurately
        let distance_sig = context_distance(&q_node.ctx_toks, &tok_info, distance);

        let q_node = &mut self.q_nodes[self.current_idx];

        q_node.score +=
            ((W_TOK * tok_sig) + (W_POS * pos_sig) + (W_STEP * distance_sig)) * q_node.rate;

        q_node.ctx_toks.push(tok_info);
    }

    // This is compensation for there being no built context window
    /// Evaluates the current node
    fn finalize_node(&mut self, end_pos: usize) {
        let q_node = &mut self.q_nodes[self.current_idx];

        q_node.end_pos = Some(end_pos);
    }

    /// Evaluates all nodes
    // More suspicious name
    fn eval(&mut self) {
        let score_logits: Vec<f32> = self.q_nodes.iter().map(|n| n.score).collect();

        let highest_score_idx =
            algo::argmax(&score_logits).expect("Quotes found are >= 2 by default");

        let highest_score = score_logits[highest_score_idx];

        // dbg!(&self.q_nodes);
        if let Some(end_idx) = self.end_tok_pos {
            let end_node = &self.q_nodes[end_idx];
        }
    }

    //WARN: Currently does not use this correctly
    fn choose_action(&self, proceed: f32, cut: f32) -> u8 {
        PROCEED
    }
}

#[derive(Debug)]
struct QuoteNode {
    start_pos: usize,
    end_pos: Option<usize>,
    score: f32,
    rate: f32,
    ctx_toks: Vec<TokenInfo>,
}

// Need variable proportional to the actual file size
impl QuoteNode {
    fn new(start_pos: usize) -> QuoteNode {
        QuoteNode {
            start_pos,
            end_pos: None,
            score: 0.05,
            rate: 0.005,
            ctx_toks: Vec::new(),
        }
    }
}
const LR: f32 = 1e-2;

/// Predicts where an unclosed quote may have started
pub fn quote_start_probability(src: &[u8], q_type: char, search_range: Range<usize>) -> Vec<Span> {
    let toks = quote_model::lexer::Lexer::new(src, &search_range, q_type).tokenize();

    let mut q_graph = QuoteGraph::new(&toks);

    // Skull emoji skull emoji skull emoji
    let mut q_model = QuoteModel::new();

    q_model.weights = vec![0.1, 0.02, 0.03];

    let test_input = vec![0.50, 0.39, 0.80];

    // Uh
    let correct_vec = vec![0.7, 0.43, 0.20];

    for i in 1..=1000 {
        let loss = train_model(&mut q_model, &test_input, &correct_vec, LR);

        if i % 5 == 0 {
            println!("step {i} | loss={loss}\n");
            dbg!(&q_model.weights);
        }
    }

    panic!("End");

    for tok_info in &toks {
        match tok_info.tok {
            Token::StrongStartQuote(pos) => q_graph.next_node(pos),
            Token::StrongEndQuote(pos) => q_graph.finalize_node(pos),
            Token::Def | Token::End | Token::Char(_) => {
                q_graph.adjust_node(tok_info.clone());
            }
            Token::EOF => break,
        }
    }

    q_graph.display_scores();

    q_graph.eval();

    q_graph.display_scores();

    // let src_str = str::from_utf8(src).unwrap();

    let mut res_idx: usize = 0;
    let mut largest_score = 0.0;

    for (i, score) in q_graph.q_nodes.iter().map(|q| q.score).enumerate() {
        if score > largest_score {
            largest_score = score;
            res_idx = i;
        }
    }

    let highest_q = &q_graph.q_nodes[res_idx];

    let start = highest_q.start_pos;

    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::new(start, start));

    // End offset is cmopensating for the tokenizer skipping to where the quotes start, rather than
    // the actual start, so end is not,
    // Inclusive exclusive + 1
    if let Some(end) = highest_q.end_pos {
        spans.push(Span::new(end, end));
    }

    spans
}

// xs
fn train_model(q_model: &mut QuoteModel, xs: &[f32], expected: &[f32], lr: f32) -> f32 {
    let mut loss_total = 0.0;

    for i in 0..xs.len() {
        let pred = q_model.predict(xs[i]);
        let error = pred - expected[i];

        loss_total += error * error;
        // Not adjusting bias right now
        let w_gradient = lr * (2.0 * error * xs[i]);

        q_model.weights[i] -= w_gradient;
    }

    loss_total
}

/// Returns the amount to apply to the signal of the given token given the context
fn context_tok(ctx_toks: &Vec<TokenInfo>, current_tok: &TokenInfo) -> f32 {
    if ctx_toks.is_empty() {
        return 1.0;
    }

    match current_tok.tok {
        Token::Char(c) if c.is_alphanumeric() => {
            let mut found = 0;
            for tok_info in ctx_toks {
                if let Token::Char(other_c) = tok_info.tok
                    && other_c.is_alphanumeric()
                {
                    found += 1;
                }
            }

            let w_adjusted = (-0.5 * found as f32).exp();
            return w_adjusted;
        }
        Token::Char(c) if c == '\n' => {
            // + 1 since steps are based on amount of characters seen and len() is off by one
            let distance = (ctx_toks.len() + 1) as f32;

            // Slow decay function so long quotes don't win over short ones just for having new
            // lines
            let w_adjusted = 1.0 / (1.0 + (0.05 * distance as f32));

            return w_adjusted;
        }
        Token::Char(c) => (),
        Token::Def => (),
        Token::End => (),
        // Handled outside
        Token::StrongStartQuote(_) => (),
        Token::StrongEndQuote(_) => (),
        Token::EOF => (),
    }

    1.0
}

fn context_distance(ctx_toks: &Vec<TokenInfo>, current_tok: &TokenInfo, distance: f32) -> f32 {
    if ctx_toks.is_empty() {
        return 1.0;
    }

    // Could be done better but, no
    match current_tok.tok {
        Token::Char(c) if c.is_alphanumeric() => (),
        Token::Char(c) if c == '\n' => {
            // Distance of new lines from start means more than everything else
            let distance_sig = 1.0 / (1.0 + (distance - 4.5).exp());
            dbg!(distance_sig);

            return distance_sig;
        }
        Token::Char(_) => (),
        Token::StrongStartQuote(_) => (),
        Token::StrongEndQuote(_) => (),
        Token::Def => (),
        Token::End => (),
        Token::EOF => (),
    }

    1.0 / (1.0 + (distance - 1.0).exp())
}

fn loss(q_model: QuoteModel) -> f32 {
    todo!();
}

//TEST:
struct QuoteModel {
    weights: Vec<f32>,
    bias: f32,
}

// Embeddings
impl QuoteModel {
    fn new() -> QuoteModel {
        QuoteModel {
            weights: Vec::new(),
            bias: 0.1,
        }
    }

    fn predict(&self, x: f32) -> f32 {
        let mut sum = 0.0;
        for i in 0..self.weights.len() {
            sum += self.weights[i] * x;
        }

        sum + self.bias
    }
}
