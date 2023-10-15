use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use matrix_bot_core::matrix::{client::Client, room::Room};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Setting {
    pub room: Vec<RoomSetting>,
    pub qbit_user: String,
    pub qbit_pass: String,
    pub qbit_url: String,
    #[cfg(target_os = "linux")]
    pub use_internal_qbit: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, Hash, Eq, PartialEq)]
pub struct RoomSetting {
    pub download_path: PathBuf,
    pub room_id: String,
    pub db_path: PathBuf,
}

impl Setting {
    pub fn get_or_init(path: impl AsRef<Path>) -> Result<Self> {
        let setting_path = path.as_ref().join("qbittorrent.toml");
        // load setting, if not exists, create it and exit
        let setting = if !setting_path.exists() {
            log::info!("create setting file: {}", setting_path.to_string_lossy());
            let setting = Setting {
                room: vec![RoomSetting {
                    download_path: path.as_ref().join("qbittorrent").join("download"),
                    room_id: "".to_string(),
                    db_path: path.as_ref().join("qbittorrent").join("db"),
                }],
                qbit_user: "admin".to_string(),
                qbit_pass: "adminadmin".to_string(),
                qbit_url: "http://127.0.0.1:8080".to_string(),
                #[cfg(target_os = "linux")]
                use_internal_qbit: true,
            };
            let toml = toml::to_string_pretty(&setting).unwrap();
            std::fs::write(&setting_path, toml)?;
            log::error!(
                "please edit setting file: {}",
                setting_path.to_string_lossy()
            );
            return Err(anyhow::anyhow!(
                "please edit setting file: {}",
                setting_path.to_string_lossy()
            ));
        } else {
            log::info!("load setting file: {}", setting_path.to_string_lossy());
            let toml = std::fs::read_to_string(&setting_path)?;
            toml::from_str(&toml)?
        };
        Ok(setting)
    }

    pub async fn to_hashmap(
        &self,
        client: &Client,
    ) -> Result<HashMap<String, (Room, RoomSetting)>> {
        let mut hashmap = HashMap::new();
        for setting in &self.room {
            let room = Room::new(client, &setting.room_id).await?;
            hashmap.insert(setting.room_id.to_string(), (room, setting.clone()));
        }
        Ok(hashmap)
    }
}
