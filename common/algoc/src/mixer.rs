//! I mix things

/// Mixer mixing mixes
pub fn fast_hash(mut v: usize) -> usize {
    v ^= v >> 23;
    v = v.wrapping_mul(0x2127599bf4325c37);
    v ^= v >> 47;
    v
}
