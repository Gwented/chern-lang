pub trait MemoryCost {
    fn cost(&self) -> usize;
}

/// Convenience function that innately accounts for string metadata and content length for the size
/// in bytes of `String`
pub fn string_cost(content: &str) -> usize {
    let metadata_cost = size_of::<String>();
    let content_cost = content.len();
    // Should this be checked?
    metadata_cost + content_cost
}

// Since deref coerced vecs would remove capacity metadata cost this is just for owned
fn generic_cost<T: MemoryCost>(thingy: &Vec<T>) -> usize {
    let metadata_cost = size_of::<Vec<T>>();
    let thingy_cost: usize = thingy.iter().map(|t| t.cost()).sum();
    // Should this be checked?
    metadata_cost + thingy_cost;
    todo!();
}
