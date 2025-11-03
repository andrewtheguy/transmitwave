use std::fs;
use std::path::PathBuf;
use transmitwave_core::*;

fn main() {
    let web_constants_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("web/src/constants/fountain.ts");

    let content = format!(
        r#"// AUTO-GENERATED FILE - DO NOT EDIT MANUALLY
// Generated from core/src/lib.rs constants
// Run `cargo run --manifest-path tools/Cargo.toml` to regenerate

export const FOUNTAIN_BLOCK_SIZE_BYTES = {}
export const MAX_PAYLOAD_BYTES = {}
export const FSK_BYTES_PER_SYMBOL = {}
export const FSK_SYMBOL_SAMPLES = {}
export const PACKET_OVERHEAD_BYTES = {}
export const MAX_BUFFER_SAMPLES = {}
"#,
        FOUNTAIN_BLOCK_SIZE,
        MAX_PAYLOAD_SIZE,
        FSK_BYTES_PER_SYMBOL,
        FSK_SYMBOL_SAMPLES,
        PACKET_OVERHEAD_BYTES,
        MAX_BUFFER_SAMPLES
    );

    fs::write(&web_constants_path, content)
        .expect("Failed to write web constants file");

    println!("Generated: {}", web_constants_path.display());
}
