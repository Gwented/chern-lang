use std::{fs, io::Read, path::PathBuf};

use common::{config_loader::ConfigLoader, metadata::FileMetadata};
use serial_lib::lexer::Lexer;

// This would be in succession to script
fn main() {
    let path = PathBuf::from("../chrn_tests/main.chrn");

    let file = fs::File::open(&path).unwrap();

    // Just so the offset can be gotten
    let metadata = ConfigLoader::new(&path, &file).load_config().unwrap();
    dbg!(metadata.serial_start);

    let lexer = Lexer::new(&metadata.src_bytes, metadata.serial_start.unwrap()).tokenize();
}
