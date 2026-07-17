use crate::id_types::{PathId, SourceRegionId};

// Should these have identifiers?
/// Byte region structure
#[derive(Debug)]
pub struct SourceRegion {
    /// Absolute starting line number of this region.
    pub abs_ln_num_start: u32,
    pub abs_col_start: u32,
    /// Index of `self`
    pub region_id: SourceRegionId,
    /// Bytes assocaited with this region, which specifically point to the script portion
    ///
    /// This is relative
    pub src_bytes: Vec<u8>,
    /// Path of this region
    pub path_id: PathId,
    // / Amount of \n within config file so binary search can be done by error reporter
    // pub new_lines: Vec<usize>,
    /// The script language start which can be different depending on if @def is used
    ///
    /// This is an absolute position.
    pub script_start: usize,
    /// The serial start, which can be `None` if there is no serialized file within the config file
    ///
    /// On `Some`, this variable is just an assumption that anything after an `@end` seen is
    /// the serial portion.
    ///
    /// This is an absolute position.
    pub serial_start: Option<usize>,
}

impl SourceRegion {
    pub fn new(
        // name_id: InternedId,
        abs_ln_num_start: u32,
        abs_col_start: u32,
        src_bytes: Vec<u8>,
        region_id: SourceRegionId,
        path_id: PathId,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> SourceRegion {
        SourceRegion {
            // new_lines: Vec::new(),
            // name_id,
            abs_ln_num_start,
            abs_col_start,
            src_bytes,
            path_id,
            region_id,
            script_start,
            serial_start,
            //TODO: Could be env var
        }
    }
}
