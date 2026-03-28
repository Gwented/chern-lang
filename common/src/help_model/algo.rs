/// Assumes the given given Vec has at least one element.
// May turn to opt
pub fn argmax(args: &Vec<f32>) -> usize {
    let mut max_idx = 0;
    let mut max_arg = args[0];

    for (i, next_arg) in args.iter().skip(1).enumerate() {
        if *next_arg > max_arg {
            max_idx = i;
            max_arg = *next_arg;
        }
    }

    max_idx
}

pub fn argmin(args: &Vec<f32>) -> usize {
    let mut min_idx = 0;
    let mut min_arg = args[0];

    for (i, next_arg) in args.iter().skip(1).enumerate() {
        if *next_arg < min_arg {
            min_idx = i;
            min_arg = *next_arg;
        }
    }

    min_idx
}
