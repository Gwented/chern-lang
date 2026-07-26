use crate::text::{abs_to_rel_offset, abs_to_rel_span, rel_to_abs_offset, rel_to_abs_span};
use chrn_utils::id_types::SourceRegionId;
use chrn_utils::source_map::source_span::SourceSpan;

#[test]
fn test_rel_abs_offset_zero_script_start() {
    for &off in &[0u32, 1, 17, 1024, u32::MAX] {
        assert_eq!(rel_to_abs_offset(off, 0), off);
        assert_eq!(abs_to_rel_offset(off, 0), off);
    }
}

#[test]
fn test_rel_to_abs_offset_basic() {
    assert_eq!(rel_to_abs_offset(0, 5), 5);
    assert_eq!(rel_to_abs_offset(3, 5), 8);
    assert_eq!(rel_to_abs_offset(100, 200), 300);
}

#[test]
fn test_abs_to_rel_offset_basic() {
    assert_eq!(abs_to_rel_offset(5, 0), 5);
    assert_eq!(abs_to_rel_offset(8, 5), 3);
    assert_eq!(abs_to_rel_offset(300, 200), 100);
    assert_eq!(abs_to_rel_offset(2, 5), 0);
}

#[test]
fn test_rel_abs_offset_roundtrip() {
    for &(rel, script) in &[(0, 5), (1, 0), (17, 200), (999, 1000)] {
        let abs = rel_to_abs_offset(rel, script);
        assert_eq!(
            abs_to_rel_offset(abs, script),
            rel,
            "roundtrip failed for rel={rel} script={script}"
        );
    }
}

#[test]
fn test_rel_to_abs_span() {
    let span = SourceSpan::new(SourceRegionId::new(0), 3, 7);
    let abs = rel_to_abs_span(span, 5);
    assert_eq!(abs.start, 8);
    assert_eq!(abs.end, 12);
    assert_eq!(abs.region_id, SourceRegionId::new(0));
}

#[test]
fn test_abs_to_rel_span() {
    let span = SourceSpan::new(SourceRegionId::new(0), 8, 12);
    let rel = abs_to_rel_span(span, 5);
    assert_eq!(rel.start, 3);
    assert_eq!(rel.end, 7);
    assert_eq!(rel.region_id, SourceRegionId::new(0));
}

#[test]
fn test_rel_to_abs_offset_boundary_values() {
    let script = (u32::MAX / 2) as usize;
    assert_eq!(rel_to_abs_offset(0, script), script as u32);
    assert_eq!(rel_to_abs_offset(1, script), (script as u32) + 1);
    assert_eq!(
        rel_to_abs_offset(u32::MAX, script),
        (script as u32).saturating_add(u32::MAX),
        "must saturate on overflow rather than wrap"
    );

    assert_eq!(rel_to_abs_offset(u32::MAX, 0), u32::MAX);

    let abs = rel_to_abs_offset(123, 456);
    assert_eq!(abs_to_rel_offset(abs, 456), 123);
}
