pub fn argmax(args: &Vec<f32>) -> Option<usize> {
    let mut max_idx = 0;
    let mut max_arg = args.get(0)?;

    for (i, next_arg) in args.iter().skip(1).enumerate() {
        if next_arg > max_arg {
            max_idx = i;
            max_arg = next_arg;
        }
    }

    Some(max_idx)
}

pub fn argmin(args: &Vec<f32>) -> Option<usize> {
    let mut min_idx = 0;
    let mut min_arg = args.get(0)?;

    for (i, next_arg) in args.iter().skip(1).enumerate() {
        if next_arg < min_arg {
            min_idx = i;
            min_arg = next_arg;
        }
    }

    Some(min_idx)
}

pub fn softmax(logits: &[f32]) -> Vec<f32> {
    let sum_exp: f32 = logits.iter().map(|val| val.exp()).sum();
    let probs: Vec<f32> = logits.iter().map(|l| l.exp() / sum_exp).collect();
    probs
}

pub fn cross_entropy(predictions: &[f32], targets: &[f32]) -> f32 {
    if predictions.len() != targets.len() {
        panic!("Length mismatch in cross entropy");
    }

    let mut loss = 0.0;
    for (pred, target) in predictions.iter().zip(targets.iter()) {
        let pred_clamped = pred.max(1e-9).min(1.0 - 1e-9);
        loss -= target * pred_clamped.ln();
    }

    loss
}

pub fn mean_squared_error(predictions: &[f32], targets: &[f32]) -> f32 {
    todo!();
}

pub fn dot_tensor1_f32(a: &Vec<f32>, b: &Vec<f32>) -> f32 {
    if a.len() != b.len() {
        panic!("Not same len in dot (Temp)");
    }

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}
