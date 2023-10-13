use std::{collections::HashMap, path::Path};

use anyhow::Result;
use matrix_bot_core::matrix::{client::Client, room::Room};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Setting {
    pub room_id: Vec<String>,
    pub token: Option<String>,
    pub port: u16,
}

impl Setting {
    pub async fn to_hashmap(&self, client: &Client) -> Result<HashMap<String, Room>> {
        let mut hashmap = HashMap::new();
        for room_id in &self.room_id {
            let room = Room::new(client, room_id).await?;
            hashmap.insert(room_id.clone(), room);
        }
        Ok(hashmap)
    }

    pub fn get_or_init(path: impl AsRef<Path>) -> Result<Self> {
        let setting_path = path.as_ref().join("webhook.toml");

        // load setting, if not exists, create it and exit
        let setting: Setting = if !setting_path.exists() {
            log::info!("create setting file: {}", setting_path.to_string_lossy());
            let settings = Setting {
                room_id: vec!["".to_string()],
                token: Some("123456".to_string()),
                port: 0,
            };
            let toml = toml::to_string_pretty(&settings).unwrap();
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
}
