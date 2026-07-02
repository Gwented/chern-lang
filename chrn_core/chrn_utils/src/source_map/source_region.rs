use crate::id_types::{PathId, SourceRegionId};

#[derive(Debug)]
pub struct SourceRegion {
    // /// Identifier associated with file that the region is in
    // pub name_id: InternedId,
    /// Index of `self`
    pub region_id: SourceRegionId,
    /// Bytes from chrn config file
    pub src_bytes: Vec<u8>,
    /// Path of this region
    pub path_id: PathId,
    // / Amount of \n within config file so binary search can be done by error reporter
    // pub new_lines: Vec<usize>,
    /// The script language start which can be different depending on if @def is used
    pub script_start: usize,
    /// The serial start which can be None if there is no serialized file within the config file
    pub serial_start: Option<usize>,
}

impl SourceRegion {
    pub fn new(
        // name_id: InternedId,
        src_bytes: Vec<u8>,
        region_id: SourceRegionId,
        path_id: PathId,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> SourceRegion {
        SourceRegion {
            // new_lines: Vec::new(),
            // name_id,
            src_bytes,
            path_id,
            region_id,
            script_start,
            serial_start,
            //TODO: Could be env var
        }
    }
}

/// Type-safe wrapper for indexing regions
#[derive(Debug)]
pub struct SourceRegionArena {
    pub regions: Vec<SourceRegion>,
}

impl SourceRegionArena {
    pub fn new(regions: Vec<SourceRegion>) -> SourceRegionArena {
        SourceRegionArena { regions }
    }

    pub fn extract_region(&self, region_id: SourceRegionId) -> &SourceRegion {
        &self.regions[region_id.id as usize]
    }

    pub fn get_region(&self, region_id: SourceRegionId) -> Option<&SourceRegion> {
        self.regions.get(region_id.id as usize)
    }
}
