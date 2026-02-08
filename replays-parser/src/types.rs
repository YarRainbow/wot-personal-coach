use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Replay {
    pub header: ReplayHeader,
    pub battle_config: BattleConfig,
    pub battle_results: Option<serde_json::Value>,
    #[serde(skip)]
    pub packets_buffer: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub magic: u32,
    pub block_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BattleConfig {
    #[serde(rename = "playerName")]
    pub player_name: String,
    #[serde(rename = "playerVehicle")]
    pub player_vehicle: String,
    #[serde(rename = "clientVersionFromXml")]
    pub client_version_xml: String,
    #[serde(rename = "clientVersionFromExe")]
    pub client_version_from_exe: String,
    #[serde(rename = "dateTime")]
    pub date_time: String,
    #[serde(rename = "mapName")]
    pub map_name: String,
    #[serde(rename = "gameplayID")]
    pub gameplay_id: String,
}
