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
