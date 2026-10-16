//! JSON Schemas for the wire format, generated from the `protocol` types the
//! same way `nat20_core::registry::schema` does it for registry files.
//! Regenerate with `cargo run -p nat20_server --example generate_schemas`;
//! a test asserts the checked-in schemas under `server/schema/` are up to
//! date.

use schemars::{JsonSchema, generate::SchemaSettings};
use serde::Serialize;
use serde_json::ser::{PrettyFormatter, Serializer};

use crate::protocol::{Request, Response, SceneDef};

/// One generated schema: its file name under `server/schema/` and the JSON
/// text.
pub struct ApiSchema {
    pub file_name: &'static str,
    pub json: String,
}

fn schema_for<T: JsonSchema>(file_name: &'static str) -> ApiSchema {
    // Draft-07, matching the registry schemas.
    let schema = SchemaSettings::draft07()
        .into_generator()
        .into_root_schema_for::<T>();

    let mut buffer = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"    ");
    let mut serializer = Serializer::with_formatter(&mut buffer, formatter);
    schema
        .serialize(&mut serializer)
        .expect("schema serialization cannot fail");
    let mut json = String::from_utf8(buffer).expect("schema JSON is valid UTF-8");
    json.push('\n');

    ApiSchema { file_name, json }
}

/// Schemas for one request line, one response line, and a scene file.
pub fn api_schemas() -> Vec<ApiSchema> {
    vec![
        schema_for::<Request>("request.schema.json"),
        schema_for::<Response>("response.schema.json"),
        schema_for::<SceneDef>("scene.schema.json"),
    ]
}
