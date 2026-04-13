use std::{fs, path::PathBuf};

use common::chern_settings::ChernSettings;
use script_lib::config_loader::ChernConfigLoader;
use serial_lib::lexer::Lexer;

// This would be in succession to script
fn main() {
    let path = PathBuf::from("../chrn_tests/main.chrn");

    let file = fs::File::open(&path).unwrap();

    // Just so the offset can be gotten
    let metadata = ChernConfigLoader::new(&path, file, &ChernSettings::new(true))
        .load_config()
        .unwrap();

    let lexer = Lexer::new(&metadata.src_bytes, metadata.serial_start.unwrap()).tokenize();
}
