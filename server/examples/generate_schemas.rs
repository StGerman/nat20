//! Writes the API JSON Schemas to `server/schema/`. Run after changing any
//! type in `protocol.rs` or `log.rs`:
//! `cargo run -p nat20_server --example generate_schemas`

use std::{fs, path::PathBuf};

use nat20_server::schema::api_schemas;

fn main() {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema");
    fs::create_dir_all(&schema_dir).expect("failed to create schema directory");

    for schema in api_schemas() {
        let path = schema_dir.join(schema.file_name);
        fs::write(&path, &schema.json)
            .unwrap_or_else(|error| panic!("failed to write {:?}: {}", path, error));
        println!("wrote {:?}", path);
    }
}
