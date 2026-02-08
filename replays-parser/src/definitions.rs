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
    pub fn new() -> Self {
        Self {
            packet_types: HashMap::new(),
            entities: HashMap::new(),
        }
    }

    /// Primary entry point for loading definitions.
    /// 1. Identifies game variant from version string (e.g. "wot_eu_...").
    /// 2. Loads default packet definitions for that variant.
    /// 3. Loads version-specific definitions (ids_{version}.json).
    /// 4. Merges them.
    pub fn load(version: &str) -> anyhow::Result<Self> {
        let mut defs = Definitions::new();

        // 1. Identify Game
        // Simple heuristic: check prefix
        let game = if version.contains("wot_eu") { "wot_eu" }
        else if version.contains("wot_ru") { "wot_ru" }
        else if version.contains("wot_na") { "wot_na" }
        else if version.contains("wot_asia") { "wot_asia" }
        else if version.contains("wot_cn") { "wot_cn" }
        else { "wot_eu" }; // Default fallback

        // 2. Load Defaults (message_codes/{game}/_default.json)
        // We look for this relative to the executable or CWD
        let default_path = std::path::Path::new("message_codes").join(game).join("_default.json");
        if default_path.exists() {
            if let Ok(d) = Self::load_from_file(&default_path) {
                defs.merge(d);
                eprintln!("Loaded defaults from {:?}", default_path);
            }
        } else {
            // Check if we are running from cargo root (development)
             let default_path_dev = std::path::Path::new("replays-parser/message_codes").join(game).join("_default.json");
             if default_path_dev.exists() {
                  if let Ok(d) = Self::load_from_file(&default_path_dev) {
                    defs.merge(d);
                    eprintln!("Loaded defaults from {:?}", default_path_dev);
                }
             }
        }

        // 3. Load Version Specific (ids_{version}.json)
        // Try file first
        let filename = format!("ids_{}.json", version);
        let path = std::path::Path::new(&filename);
        
        let mut version_defs = None;
        if path.exists() {
            if let Ok(d) = Self::load_from_file(path) {
                 version_defs = Some(d);
                 eprintln!("Loaded overrides from {:?}", path);
            }
        }
        
        // Try embedded if file not found
        if version_defs.is_none() {
            if let Some(d) = Self::load_embedded(version) {
                version_defs = Some(d);
                eprintln!("Loaded embedded definitions for {}", version);
            }
        }

        if let Some(d) = version_defs {
            defs.merge(d);
        }

        Ok(defs)
    }

    /// Merges other into self. Overwrites conflicting keys.
    pub fn merge(&mut self, other: Definitions) {
        // Merge Packet Types
        for (k, v) in other.packet_types {
            // merge logic for packet types?
            // If it's a simple KV, overwrite.
            // If it's a nested object (subtypes), ideally deep merge, but for now strict overwrite is safer/simpler
            // unless we want to support partial updates.
            // Given the JSON structure: "0x08": { id: ..., subtypes: ... }
            // Let's just overwrite for now.
            self.packet_types.insert(k, v);
        }

        // Merge Entities
        for (k, v) in other.entities {
            self.entities.insert(k, v);
        }
    }

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
