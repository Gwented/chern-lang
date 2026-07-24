//! Lower

use std::path::Path;

use chrn_utils::{
    arena::Arena,
    chrn_config::ChrnConfig,
    core_error::{ConfigLoadError, ModuleInitError},
    files::file_ops,
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
    },
};
use compilation::config_loader::{ConfigLoader, ConfigLoaderOutput};

pub fn run_cfg_loader(path: &Path, cfg: &ChrnConfig) -> Result<SourceRegion, ModuleInitError> {
    // let interner = Intern::init();
    // let src = match file_ops::fopen(path) {
    //     Ok(f) => f,
    //     Err(err_msg) => {
    //         // Interning mangled path id so it can still go into the diagnostic
    //         let path_id = interner.intern_path(path);
    //         let src_diag =
    //             SourceDiagnostic::builder(None, DiagnosticLevel::Error, err_msg, path_id).build();
    //         let cfg_err = ConfigLoadError::Diagnostic(src_diag);
    //
    //         return Err(ModuleInitError::new(None, interner, cfg_err));
    //     }
    // };
    //
    // let main_path_id = interner.intern_path(&path);
    //
    // let mut region_arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
    // let main_region_id = SourceRegionId::new(0);
    //
    // // Not sure if main should even recover from this beyond having an existent region
    // match ConfigLoader::new(main_region_id, src, main_path_id, &cfg, &interner).load_config() {
    //     ConfigLoaderOutput::Success(region) => Ok(ConfigLoaderOutput::Success(region)),
    //     // This could be pretty bad to leave here because
    //     ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
    //         // Odd handling..
    //         let diag = match cfg_err {
    //             ConfigLoadError::Diagnostic(diag) => diag,
    //             ConfigLoadError::IO(io_err) => {
    //                 let err_str = core_error::form_string_from_io_err(&io_err, path)
    //                     .unwrap_or(io_err.to_string());
    //                 SourceDiagnostic::builder(
    //                     //TODO: Should this have a code?
    //                     None,
    //                     DiagnosticLevel::Error,
    //                     err_str,
    //                     main_path_id,
    //                 )
    //                 .build()
    //             }
    //         };
    //
    //         diags.push(diag);
    //         Ok(ConfigLoaderOutput::Broken(broken_region, cfg_err))
    //     }
    //     ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
    //         Err(ModuleInitError::new(None, interner, cfg_err))
    //     }
    // }
    todo!()
}
