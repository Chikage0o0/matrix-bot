use std::path::Path;

use anyhow::Result;
use matrix_bot_core::matrix::client::Client;

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
        for (setting, (db, room)) in setting_hashmap.iter() {
            log::info!("scan: {}", setting.room_id);
            let mut image_list = Vec::new();

            for url in setting.yande_url.iter() {
                let list = yande::get_image_list(url).await?;
                image_list.extend(list);
            }

            let download_list = yande::get_download_list(&image_list, &db).await?;

            for (id, img_data) in download_list {
                let msg = format!(
                    "来源：[https://yande.re/post/show/{id}](https://yande.re/post/show/{id})"
                );
                room.send_msg(&msg, true)
                    .await
                    .unwrap_or_else(|e| log::error!("send msg failed: {}", e));
                for (id, url) in img_data.url.iter() {
                    log::info!("prepare download: {}", id);
                    let path = match yande::download_img(*id, url, &setting.tmp_path).await {
                        Ok(path) => path,
                        Err(e) => {
                            log::error!("download failed: {}", e);
                            continue;
                        }
                    };

                    let path = if let Some(size) = setting.resize {
                        match resize::resize_and_compress(&path, size) {
                            Ok(path) => path,
                            Err(e) => {
                                log::error!("resize {id} failed: {}", e);
                                continue;
                            }
                        }
                    } else {
                        path
                    };

                    log::info!("upload: {}", id);

                    room.send_attachment(&path)
                        .await
                        .unwrap_or_else(|e| log::error!("send attachment failed: {}", e));

                    std::fs::remove_file(&path).unwrap_or_else(|e| {
                        log::error!("remove file failed: {}", e);
                    });
                }
            }
            log::info!("scan: {} done", setting.room_id);
            db.auto_remove()?;
        }

        tokio::time::sleep(tokio::time::Duration::from_secs(60 * 60)).await;
    }
}
