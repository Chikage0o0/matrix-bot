use std::{collections::HashMap, path::Path};

use anyhow::Result;
use db::DB;
use matrix_bot_core::matrix::{client::Client, room::Room};
use setting::RoomSetting;

use crate::setting::Setting;

mod db;
mod resize;
mod setting;
mod yande;

pub async fn run(client: Client, plugin_folder: impl AsRef<Path>) -> Result<()> {
    log::info!("start yande_popular");

    let setting_hashmap = Setting::get_or_init(plugin_folder)?
        .to_hashmap(&client)
        .await?;

    loop {
        log::info!("start scan");
        sync(&setting_hashmap).await.unwrap_or_else(|e| {
            log::error!("scan failed: {}", e);
        });
        tokio::time::sleep(tokio::time::Duration::from_secs(60 * 60)).await;
    }
}

pub async fn sync(setting_hashmap: &HashMap<RoomSetting, (DB, Room)>) -> Result<()> {
    for (setting, (db, room)) in setting_hashmap.iter() {
        log::info!("scan: {}", setting.room_id);
        let mut image_list = Vec::new();

        for url in setting.yande_url.iter() {
            let list = yande::get_image_list(url).await?;
            image_list.extend(list);
        }

        let download_list = yande::get_download_list(&image_list, &db).await?;

        for (id, img_data) in download_list {
            for (id, url) in img_data.url.iter() {
                log::info!("prepare download: {}", id);
                let path = match yande::download_img(*id, url, &setting.tmp_path).await {
                    Ok(path) => path,
                    Err(e) => {
                        log::error!("download failed: {}", e);
                        return Err(e.into());
                    }
                };

                let path = if let Some(size) = setting.resize {
                    match resize::resize_and_compress(&path, size) {
                        Ok(path) => path,
                        Err(e) => {
                            log::error!("resize {id} failed: {}", e);
                            return Err(e.into());
                        }
                    }
                } else {
                    path
                };

                log::info!("upload: {}", id);

                match room.send_attachment(&path).await {
                    Ok(_) => {
                        log::info!("upload: {} done", id);
                        db.insert(&id.to_string())?;
                    }
                    Err(e) => {
                        log::error!("upload failed: {}", e);
                        return Err(e.into());
                    }
                }

                std::fs::remove_file(&path).unwrap_or_else(|e| {
                    log::error!("remove file failed: {}", e);
                });
            }
            let msg =
                format!("来源：[https://yande.re/post/show/{id}](https://yande.re/post/show/{id})");
            room.send_msg(&msg, true).await?;
        }
        log::info!("scan: {} done", setting.room_id);
        db.auto_remove()?;
    }
    Ok(())
}
