//TODO: Should be able to truncate current pos + 1 (cut) and reposition self given something like
//@end twice, meaning it should probably go to the node at the first @end

// None of this is supposed to be efficient since 1, this will rarely ever happen, and 2, learning.
mod lexer;
mod token;

use std::ops::Range;

use crate::{
    help_model::{
        algo,
        math_structs::Tensor2,
        quote_model::{
            self,
            token::{Token, TokenInfo},
        },
    },
    symbols::Span,
};

const W_TOK: f32 = 0.6;
const W_POS: f32 = 1.0;
const W_DIST: f32 = 3.5;

/// Basic state of not needing any special actions
const PROCEED: u8 = 0;
/// Action to cut portion of array
const CUT: u8 = 1;

#[derive(Debug)]
pub struct QuoteGraph<'a> {
    pub(crate) current_idx: usize,
    pub(crate) q_nodes: Vec<QuoteNode>,
    pub(crate) ctx_toks: &'a Vec<TokenInfo>,
}

impl QuoteGraph<'_> {
    // Um...
    fn init(ctx_toks: &Vec<TokenInfo>) -> QuoteGraph<'_> {
        let mut q_graph = QuoteGraph {
            current_idx: 0,
            q_nodes: Vec::new(),
            ctx_toks,
        };

        q_graph.construct_nodes();

        q_graph
    }

    fn display_scores(&self) {
        for (i, q_node) in self.q_nodes.iter().enumerate() {
            println!(
                "Node {i}: \nstart: {} | end: {:?}\nscore: {} ",
                q_node.src_start_pos, q_node.src_end_pos, q_node.score
            );
        }
    }

    fn construct_nodes(&mut self) {
        let mut src_start = 0;
        let mut ctx_tok_start = 0;
        let mut in_quotes = false;

        for (ctx_tok_pos, tok_info) in self.ctx_toks.iter().enumerate() {
            match tok_info.tok {
                Token::StrongStartQuote(start) => {
                    in_quotes = true;
                    ctx_tok_start = ctx_tok_pos;
                    src_start = start;
                }
                Token::StrongEndQuote(src_end) => {
                    // Inclusive so it also gets the end quote in the current iteration
                    let ctx_toks = self.ctx_toks[ctx_tok_start..=ctx_tok_pos].to_vec();
                    let q_node = QuoteNode::new(src_start, Some(src_end), ctx_toks);
                    self.q_nodes.push(q_node);

                    in_quotes = false;
                }
                Token::EOF => {
                    if in_quotes {
                        let ctx_toks = self.ctx_toks[ctx_tok_start..].to_vec();
                        let q_node = QuoteNode::new(src_start, None, ctx_toks);

                        self.q_nodes.push(q_node);
                    }

                    break;
                }
                _ => (),
            }
        }
    }

    /// Runs probabilities to check if any node is structurally likely to be a cascaded quote,
    /// rather than the source of the unclosed quote. Removes any that are thought to be cascaded.
    fn validate_cascade(&mut self) {
        for q_node in &mut self.q_nodes {
            let cascade_prob = q_node.predict_location();
        }
    }

    /// Creates a new node and sets it as it's current to be adjusted
    fn next_node(&mut self) {
        self.current_idx = self.q_nodes.len();
    }

    fn adjust_node(&mut self, tok_info: TokenInfo) {
        let q_node = &self.q_nodes[self.current_idx];

        let start_pos = q_node.src_start_pos as f32;

        let distance = (q_node.ctx_toks.len() + 1) as f32;

        // if tok_info.tok == Token::End {
        //     self.end_tok_pos = Some(self.current_idx);
        // }

        let tok_sig = tok_info.sig * context_tok(&q_node.ctx_toks, &tok_info);

        // Exponential decrease of positional bias as the starting position increases
        let pos_sig = (-(2.0f32.ln()) * (start_pos + 0.20)).exp();

        // Extreme bias towards being at the beginning to catch """ more accurately
        let distance_sig = context_distance(&q_node.ctx_toks, &tok_info, distance);

        let q_node = &mut self.q_nodes[self.current_idx];

        q_node.score +=
            ((W_TOK * tok_sig) + (W_POS * pos_sig) + (W_DIST * distance_sig)) * q_node.rate;

        q_node.ctx_toks.push(tok_info);
    }

    /// Evaluates the current node
    fn finalize_node(&mut self, end_pos: usize) {
        let q_node = &mut self.q_nodes[self.current_idx];

        q_node.src_end_pos = Some(end_pos);
    }

    /// Evaluates all nodes
    // More suspicious name
    fn eval(&mut self) {
        let score_logits: Vec<f32> = self.q_nodes.iter().map(|n| n.score).collect();

        let highest_score_idx =
            algo::argmax(&score_logits).expect("Quotes found are >= 2 by default");

        let highest_score = score_logits[highest_score_idx];

        // dbg!(&self.q_nodes);
        // if let Some(end_idx) = self.end_tok_pos {
        //     let end_node = &self.q_nodes[end_idx];
        // }
    }

    //WARN: Currently does not use this correctly
    fn choose_action(&self, proceed: f32, cut: f32) -> u8 {
        PROCEED
    }
}

#[derive(Debug)]
pub(crate) struct QuoteNode {
    src_start_pos: usize,
    src_end_pos: Option<usize>,
    score: f32,
    rate: f32,
    ctx_toks: Vec<TokenInfo>,
}

impl QuoteNode {
    fn new(
        src_start_pos: usize,
        src_end_pos: Option<usize>,
        ctx_toks: Vec<TokenInfo>,
    ) -> QuoteNode {
        QuoteNode {
            src_start_pos,
            src_end_pos,
            score: 0.05,
            rate: 0.005,
            ctx_toks,
        }
    }

    // in a valid set of quotes
    // in an invalid set of qoutes
    // in a cascaded quote
    fn predict_location(&self) -> f32 {
        let cascade: f32 = 0.0;

        for tok_info in &self.ctx_toks {
            match tok_info.tok {
                Token::Def => todo!(),
                Token::Char(_) => todo!(),
                Token::End => todo!(),
                Token::StrongStartQuote(_) => (),
                Token::StrongEndQuote(_) | Token::EOF => break,
            }
        }

        todo!();
    }
}
const LR: f32 = 1e-2;

/// Predicts where an unclosed quote may have started
pub fn quote_start_probability(src: &[u8], q_type: char, search_range: Range<usize>) -> Vec<Span> {
    let toks = quote_model::lexer::Lexer::new(src, &search_range, q_type).tokenize();

    // let embeddings: Vec<Vec<f32>> = algo::make_randomized_tensor1(5);

    // What
    // 0 = Other, 1 = \n, 3 = alphanum
    let embeddings = Tensor2::from(&vec![
        // OBracket
        vec![0.8, 0.3],
        // CBracket
        vec![0.25, 0.15],
        //
        // vec![0.9, 0.12],
    ]);

    let weights = Tensor2::from(&vec![vec![0.8, 0.3], vec![0.25, 0.15]]);
    let mut q_model = QuoteModel::with_presets(weights, embeddings);

    // "[[]"
    let input: Vec<usize> = vec![0, 0, 1];

    // Expected index guess for missing bracket
    let expected: usize = 1;

    for i in 1..=500 {
        let (loss, gradients) = train_model(&q_model, &input, expected, LR);

        if i % 100 == 0 {
            println!("step {i} | loss={loss}\n");
        }
    }

    let mut q_graph = QuoteGraph::init(&toks);

    // DOES NOTHING
    q_graph.eval();

    // q_graph.display_scores();

    let scores: Vec<f32> = q_graph.q_nodes.iter().map(|q| q.score).collect();
    let highest_idx = algo::argmax(&scores).expect("temp");

    let highest_q_node = &q_graph.q_nodes[highest_idx];

    let mut spans: Vec<Span> = Vec::new();

    spans.push(Span::new(
        highest_q_node.src_start_pos,
        highest_q_node.src_start_pos,
    ));

    if let Some(pos) = highest_q_node.src_end_pos {
        spans.push(Span::new(pos, pos));
    }

    spans
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

    match current_tok.tok {
        Token::Char(c) if c.is_alphanumeric() => (),
        Token::Char(c) if c == '\n' => {
            // Distance of new lines from start means more than everything else
            let distance_sig = 1.0 / (1.0 + (distance - 4.5).exp());

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
#[derive(Debug)]
struct QuoteModel {
    weights: Tensor2<f32>,
    embeddings: Tensor2<f32>,
    bias: f32,
}

impl QuoteModel {
    fn with_presets(weights: Tensor2<f32>, embeddings: Tensor2<f32>) -> QuoteModel {
        QuoteModel {
            weights,
            embeddings,
            bias: 0.1,
        }
    }

    // fn with_random() -> QuoteModel {
    //     QuoteModel {
    //         weights: Tensor2::with_random(5, 5, 0.0..0.9),
    //         bias: 0.1,
    //     }
    // }
}

//TEST: Learning
fn train_model(
    q_model: &QuoteModel,
    // Token ids
    inputs: &Vec<usize>,
    expected: usize,
    lr: f32,
) -> (f32, usize) {
    let embedding_table = &q_model.embeddings;

    let inputs: Vec<&[f32]> = vec![
        // 0
        embedding_table.get_row(inputs[0]),
        // 0
        embedding_table.get_row(inputs[1]),
        // 1
        embedding_table.get_row(inputs[2]),
    ];

    let mut avgs: Vec<f32> = Vec::with_capacity(inputs.len());

    for i in 0..inputs.len() {
        avgs.push(0.0);
        for j in 0..embedding_table.cols {
            avgs[i] += inputs[i][j];
        }

        avgs[i] /= inputs.len() as f32;
    }

    let mut preds: Vec<f32> = Vec::new();

    // Imagine
    for row in 0..q_model.weights.rows {
        let mut sum: f32 = 0.0;

        for col in 0..q_model.weights.cols {
            let weight = q_model.weights.get(row, col);

            for i in 0..inputs.len() {
                sum += weight * inputs[i][col];
            }
        }

        preds.push(sum + q_model.bias);
    }

    let probs = algo::softmax(&preds);
    dbg!(&preds);

    let choice = algo::argmax(&probs).expect("No");
    panic!();

    let n = q_model.weights.inner.len() as f32;
    // let mut gradients_inner: Vec<f32> = Vec::with_capacity(q_model.weights.inner.len());

    // for (pred, target) in q_model.weights.inner.iter().zip(expected.inner.iter()) {
    //     let grad = (pred - target) / n;
    //     gradients_inner.push(grad);
    // }

    // let gradients = Tensor2 {
    //     inner: gradients_inner,
    //     rows: predictions.rows,
    //     cols: predictions.cols,
    // };

    // for i in 0..q_model.weights.inner.len() {
    //     q_model.weights.inner[i] -= lr * gradients.inner[i];
    // }

    // (loss, gradients)
    (0.0, choice)
}
