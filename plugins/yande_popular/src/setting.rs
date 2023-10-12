use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Result;
use matrix_bot_core::matrix::{client::Client, room::Room};
use serde::{Deserialize, Serialize};

use crate::db::DB;

#[derive(Debug, Deserialize, Serialize, Clone, Hash, Eq, PartialEq)]
pub struct RoomSetting {
    pub tmp_path: PathBuf,
    pub db_path: PathBuf,
    pub room_id: String,
    pub resize: Option<usize>,
    pub yande_url: Vec<String>,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Setting {
    room: Vec<RoomSetting>,
}

impl Setting {
    pub async fn to_hashmap(self, client: &Client) -> Result<HashMap<RoomSetting, (DB, Room)>> {
        let mut hashmap = HashMap::new();
        for setting in self.room {
            let db = DB::open(&setting.db_path);
            std::fs::create_dir_all(Path::new(&setting.tmp_path)).unwrap_or_else(|e| {
                log::error!("create tmp dir failed: {}", e);
            });
            let room = Room::new(&client, &setting.room_id).await?;
            hashmap.insert(setting.clone(), (db, room));
        }
        Ok(hashmap)
    }

    pub fn get_or_init(path: impl AsRef<Path>) -> Result<Self> {
        let setting_path = path.as_ref().join("yande_popular.toml");
        // load setting, if not exists, create it and exit
        let settings: Self = if !setting_path.exists() {
            log::info!("create setting file: {}", setting_path.to_string_lossy());
            let settings = Setting {
                room: vec![RoomSetting {
                    tmp_path: path.as_ref().join("yande_popular").join("tmp"),
                    db_path: path.as_ref().join("yande_popular").join("db"),
                    room_id: "".to_string(),
                    resize: Some(1920),
                    yande_url: vec!["https://yande.re/post/popular_recent".to_string()],
                }],
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
        Ok(settings)
    }
}
