use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Include the generated code from build.rs
// This file will contain `pub fn get_definitions_json(version: &str) -> Option<&'static str>`
include!(concat!(env!("OUT_DIR"), "/generated_ids.rs"));

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Definitions {
    #[serde(rename = "packetTypes")]
    pub packet_types: HashMap<String, serde_json::Value>, 
    pub entities: HashMap<String, EntityDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EntityDef {
    pub id: u32,
    pub name: String,
    #[serde(rename = "clientMethods")]
    pub client_methods: HashMap<String, MethodDef>,
    pub properties: HashMap<String, PropertyDef>,
    #[serde(rename = "cellMethods")]
    pub cell_methods: HashMap<String, MethodDef>,
    #[serde(rename = "baseMethods")]
    pub base_methods: HashMap<String, MethodDef>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MethodDef {
    pub name: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PropertyDef {
    pub name: String,
    #[serde(default)]
    pub r#type: String, // 'type' is a reserved keyword
}

impl Definitions {
    /// Loads definitions for a specific version from the embedded JSON.
    /// Returns None if version not found.
    pub fn load_embedded(version: &str) -> Option<Self> {
        let json_str = get_definitions_json(version)?;
        match serde_json::from_str(json_str) {
            Ok(defs) => Some(defs),
            Err(e) => {
                eprintln!("Error parsing embedded definitions for {}: {}", version, e);
                None
            }
        }
    }

    /// Loads definitions from a JSON file.
    pub fn load_from_file(path: &std::path::Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let defs = serde_json::from_reader(reader)?;
        Ok(defs)
    }
}
