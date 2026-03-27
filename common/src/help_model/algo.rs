/// Assumes the given given Vec has at least one element.
fn argmax(args: &Vec<f32>) -> Option<usize> {
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
