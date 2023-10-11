use std::{collections::HashMap, net::SocketAddr, sync::OnceLock};

use anyhow::Result;
use axum::{extract::Path, http::StatusCode, routing::post, Json, Router};
use matrix_bot_core::matrix::{
    client::Client,
    room::{self, Room},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct Setting {
    room_id: Vec<String>,
    token: Option<String>,
    port: u16,
}

static ROOM: OnceLock<HashMap<String, Room>> = OnceLock::new();

pub async fn run(client: Client, setting_folder: impl AsRef<std::path::Path>) -> Result<()> {
    log::info!("start webhook");
    let setting_path = setting_folder.as_ref().join("webhook.toml");

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

    // map to hashmap for easy access
    // key: room_id Value: Room
    let mut setting_map = HashMap::new();
    for room_id in setting.room_id {
        let room = room::Room::new(&client, &room_id).await?;
        setting_map.insert(room_id, room);
    }

    ROOM.get_or_init(|| setting_map);

    let mut app: Router = Router::new()
        .route("/send/:room_id", post(send))
        .fallback(not_found);

    if let Some(token) = &setting.token {
        app = app.layer(tower_http::validate_request::ValidateRequestHeaderLayer::bearer(token));
    }

    log::info!("listen on {}", setting.port);

    let addr = SocketAddr::from(([0, 0, 0, 0], setting.port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    server.await.unwrap();

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct Msg {
    msg: String,
}
async fn send((Path(room_id), Json(msg)): (Path<String>, Json<Msg>)) -> StatusCode {
    let room = ROOM.get().unwrap();
    if let Some(room) = room.get(&room_id) {
        log::info!("send msg: {}", msg.msg);
        match room.send_msg(&msg.msg, true).await {
            Ok(_) => {
                log::info!("send msg success");
                StatusCode::OK
            }
            Err(e) => {
                log::error!("send msg failed: {}", e);
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    } else {
        log::error!("room not found: {}", room_id);
        StatusCode::NOT_FOUND
    }
}

async fn not_found() -> StatusCode {
    StatusCode::NOT_FOUND
}
