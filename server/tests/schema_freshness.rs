//! Asserts the checked-in `server/schema/*.schema.json` files match the
//! current `protocol` types, so a wire format change fails loudly instead of
//! silently drifting from the clients built against the schemas.

use std::path::PathBuf;

use nat20_server::schema::api_schemas;

#[test]
fn checked_in_schemas_are_up_to_date() {
    let schema_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("schema");

    for schema in api_schemas() {
        let path = schema_dir.join(schema.file_name);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {:?}: {}", path, error))
            .replace("\r\n", "\n");

        assert_eq!(
            on_disk, schema.json,
            "{:?} is stale; regenerate with `cargo run -p nat20_server --example generate_schemas`",
            path
        );
    }
}
