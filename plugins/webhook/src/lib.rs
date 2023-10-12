use std::{collections::HashMap, net::SocketAddr};

use anyhow::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use matrix_bot_core::matrix::{client::Client, room::Room};

use crate::setting::Setting;
mod setting;

pub async fn run(client: Client, setting_folder: impl AsRef<std::path::Path>) -> Result<()> {
    log::info!("start webhook");

    let setting = Setting::get_or_init(setting_folder)?;
    let token = setting.token.clone();
    let port = setting.port;
    let setting = setting.to_hashmap(&client).await?;

    let mut app = Router::new()
        .route("/send/:room_id", post(send))
        .fallback(not_found)
        .with_state(setting);

    if let Some(token) = &token {
        app = app.layer(tower_http::validate_request::ValidateRequestHeaderLayer::bearer(token));
    }

    log::info!("listen on {}", port);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let server = axum::Server::bind(&addr).serve(app.into_make_service());

    server.await.unwrap();

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct Msg {
    msg: String,
}
async fn send(
    State(room): State<HashMap<String, Room>>,
    Path(room_id): Path<String>,
    Json(msg): Json<Msg>,
) -> StatusCode {
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
