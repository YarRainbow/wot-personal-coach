use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::collections::HashMap;

// We need to duplicate the structs here to deserialize the JSON
// because we can't import them from src/definitions.rs easily in build.rs
#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct Definitions {
    #[serde(rename = "packetTypes")]
    packet_types: HashMap<String, serde_json::Value>, 
    entities: HashMap<String, EntityDef>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct EntityDef {
    id: u32,
    name: String,
    // Maps ID string -> MethodDef or PropertyDef
    #[serde(rename = "clientMethods")]
    client_methods: HashMap<String, MethodDef>,
    properties: HashMap<String, PropertyDef>,
    #[serde(rename = "cellMethods")]
    cell_methods: HashMap<String, MethodDef>,
    #[serde(rename = "baseMethods")]
    base_methods: HashMap<String, MethodDef>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct MethodDef {
    name: String,
    // args, etc. ignored for now for the lookup map, we just need names
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct PropertyDef {
    name: String,
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("generated_ids.rs");
    let mut file = BufWriter::new(File::create(&dest_path).unwrap());

    // 1. Scan for ids_*.json files
    // We look in the crate root (where Cargo.toml is)
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let root = Path::new(&crate_dir);
    
    // Rerun if any json file changes
    println!("cargo:rerun-if-changed=build.rs");
    
    // We want to map "version_string" -> Definitions struct (but Definitions is complex)
    // We will generate a function `get_definitions_json(version: &str) -> Option<&'static str>`
    // using a valid match statement.
    
    let mut versions = Vec::new();

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if name.starts_with("ids_") && name.ends_with(".json") {
                    println!("cargo:rerun-if-changed={}", name);
                    
                    // ids_wot_eu_v1_25_1_0.json -> version = "wot_eu_v1_25_1_0"
                    let version = name.trim_start_matches("ids_").trim_end_matches(".json");
                    
                    // Parse the JSON
                    let file_reader = File::open(&path).expect("failed to open json");
                    let defs: Definitions = serde_json::from_reader(file_reader).expect("failed to parse json");
                    
                    versions.push((version.to_string(), defs));
                }
            }
        }
    }

    // Sort for deterministic output
    versions.sort_by(|a, b| a.0.cmp(&b.0));

    // Generate static functions to get data
    // pub fn get_definitions_json(version: &str) -> Option<&'static str> {
    //    match version {
    //       "wot_eu_v1_25_1_0" => Some(r#"{...}"#),
    //       ...
    //       _ => None,
    //    }
    // }

    write!(&mut file, "pub fn get_definitions_json(version: &str) -> Option<&'static str> {{\n").unwrap();
    write!(&mut file, "    match version {{\n").unwrap();
    for (ver, defs) in &versions {
        let json_str = serde_json::to_string(defs).expect("failed to serialize");
        // Escape appropriately for raw string literal if needed, but r#""# usually handles standard JSON well
        // unless it contains "# which is rare in this data.
        write!(&mut file, "        \"{}\" => Some(r#\"{}\"#),\n", ver, json_str).unwrap();
    }
    write!(&mut file, "        _ => None,\n").unwrap();
    write!(&mut file, "    }}\n").unwrap();
    write!(&mut file, "}}\n").unwrap();
}

