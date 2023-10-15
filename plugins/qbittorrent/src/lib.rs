use std::{collections::HashMap, sync::OnceLock};

use anyhow::Result;
use matrix_bot_core::matrix::{client::Client, room::Room};
use qbit_rs::Qbit;
use setting::RoomSetting;

use crate::{
    qbit::ops::{expire_torrents, upload_torrents},
    setting::Setting,
};

mod matrix;
mod qbit;
mod setting;
mod upload;

static ROOM_MAP: OnceLock<HashMap<String, (Room, RoomSetting)>> = OnceLock::new();
static API: OnceLock<Qbit> = OnceLock::new();

#[allow(unused_variables)]
pub async fn run(client: Client, plugin_folder: impl AsRef<std::path::Path>) -> Result<()> {
    log::info!("start qbittorrent plugin");

    let setting = Setting::get_or_init(plugin_folder)?;

    #[cfg(target_os = "linux")]
    let _child: std::process::Child;
    #[cfg(target_os = "linux")]
    if setting.use_internal_qbit {
        let runtime_folder = std::path::PathBuf::from("data/plugins/qbittorrent/runtime");
        let port = setting.qbit_url.split(":").last();
        let port = port.unwrap_or("80").parse().unwrap_or(80);
        _child = qbit::binary::run(&runtime_folder, port)?;
    }

    let room = setting.to_hashmap(&client).await?;
    ROOM_MAP
        .set(room)
        .map_err(|_| anyhow::anyhow!("ROOM_MAP OnceLock double set"))?;

    let api = qbit::ops::login(&setting.qbit_user, &setting.qbit_pass, &setting.qbit_url).await?;
    API.set(api)
        .map_err(|_| anyhow::anyhow!("API OnceLock double set"))?;

    for (room, setting) in ROOM_MAP.get().unwrap().values() {
        matrix::add_listener(room);
    }

    loop {
        let (expire, upload) = qbit::ops::scan_torrent(API.get().unwrap()).await?;

        expire_torrents(API.get().unwrap(), &expire)
            .await
            .unwrap_or_else(|e| {
                log::error!("expire torrent failed: {}", e);
            });

        upload_torrents(API.get().unwrap(), &upload)
            .await
            .unwrap_or_else(|e| {
                log::error!("upload torrent failed: {}", e);
            });

        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
