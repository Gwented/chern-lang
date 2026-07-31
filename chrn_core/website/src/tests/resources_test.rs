//! Tests for `src/resources.rs`.

use crate::resources::{error_page_href, errors_index_href, landing_href};

/// Depths must match where the generator writes each page.
#[test]
fn hrefs_match_page_depth() {
    assert_eq!(landing_href("clip.mp4"), "resources/clip.mp4");
    assert_eq!(errors_index_href("clip.mp4"), "../resources/clip.mp4");
    assert_eq!(error_page_href("clip.mp4"), "../../resources/clip.mp4");
}
