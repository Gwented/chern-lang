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

pub fn softmax(logits: &Vec<f32>) -> Vec<f32> {
    let sum_exp: f32 = logits.iter().map(|val| val.exp()).sum();
    let probs: Vec<f32> = logits.iter().map(|l| l.exp() / sum_exp).collect();
    probs
}
