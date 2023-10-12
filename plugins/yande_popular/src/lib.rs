use std::{
    collections::HashMap,
    hash::Hash,
    path::{self, Path, PathBuf},
};

use anyhow::Result;
use matrix_bot_core::matrix::{client::Client, room};
use serde::{Deserialize, Serialize};

mod db;
mod resize;
mod yande;

#[derive(Debug, Deserialize, Serialize)]
struct Settings {
    setting: Vec<Setting>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Hash, Eq, PartialEq)]
struct Setting {
    tmp_path: PathBuf,
    db_path: PathBuf,
    room_id: String,
    resize: Option<usize>,
    yande_url: Vec<String>,
}

pub async fn run(client: Client, setting_folder: impl AsRef<Path>) -> Result<()> {
    log::info!("start yande_popular");
    let setting_path = setting_folder.as_ref().join("yande_popular.toml");

    // load setting, if not exists, create it and exit
    let settings: Settings = if !setting_path.exists() {
        log::info!("create setting file: {}", setting_path.to_string_lossy());
        let settings = Settings {
            setting: vec![Setting {
                tmp_path: setting_folder.as_ref().join("yande_popular").join("tmp"),
                db_path: setting_folder.as_ref().join("yande_popular").join("db"),
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

    // map to hashmap for easy access
    // key: Setting Value: (DB, Room)
    let mut settings_hash = HashMap::new();
    for setting in settings.setting {
        let db = db::DB::open(&setting.db_path);
        std::fs::create_dir_all(path::Path::new(&setting.tmp_path)).unwrap_or_else(|e| {
            log::error!("create tmp dir failed: {}", e);
        });
        let room = room::Room::new(&client, &setting.room_id).await?;
        settings_hash.insert(setting.clone(), (db, room));
    }

    loop {
        log::info!("start scan");
        for (setting, (db, room)) in settings_hash.iter() {
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
