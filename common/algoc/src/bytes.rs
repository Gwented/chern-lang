/// Byte similarity check
pub fn is_similar(a: &[u8], b: &[u8]) -> bool {
    let mut chances = 2;
    let mut matched = 0;

    let size_diff = a.len().max(b.len()) - a.len().min(b.len());

    if size_diff > 3 {
        return false;
    }

    let cap = a.len().min(b.len());

    for j in 0..cap {
        if a[j] == b[j] {
            matched += 1;
            chances = 1;
        } else if chances == 0 {
            break;
        } else {
            chances -= 1;
        }
    }

    // How about len dependent matching?
    // Edit distance checking?
    if matched > 2 || (matched >= 2 && matched + 1 >= b.len()) {
        return true;
    }

    false
}
