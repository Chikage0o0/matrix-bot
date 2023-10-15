use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use matrix_bot_core::{
    matrix::{self, client::Client},
    matrix_sdk::config::SyncSettings,
};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Home Server URL
    #[arg(short = 's', long, env = "HOMESERVER_URL")]
    homeserver_url: String,

    /// Matrix username
    #[arg(short, long, env = "USERNAME")]
    username: String,

    /// Matrix password
    #[arg(short, long, env = "PASSWORD")]
    password: String,

    /// Data folder
    #[arg(short, long, env = "DATA_PATH", default_value = "data")]
    data: PathBuf,
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter(None, log::LevelFilter::Info)
        .filter(Some("matrix_sdk"), log::LevelFilter::Warn)
        .filter(Some("tracing"), log::LevelFilter::Warn)
        .init();

    let args = Args::parse();

    let matrix_client = matrix::client::Client::login(
        &args.homeserver_url,
        &args.username,
        &args.password,
        &args.data.join("session.json"),
        &args.data.join("db"),
    )
    .await
    .unwrap();
    let mut event_handlers = Vec::new();
    let e2ee_sync = matrix::e2ee::sync(&matrix_client).unwrap();

    event_handlers.extend(e2ee_sync.0);

    let _ = load_plugins(&matrix_client, args.data.join("plugins"));

    let ctrlc = tokio::signal::ctrl_c();

    let client = matrix_client.clone();

    let handle = tokio::spawn(async move {
        client
            .sync(SyncSettings::new().timeout(std::time::Duration::from_secs(30)))
            .await
    });

    tokio::select! {
        _=ctrlc => {
            log::info!("Ctrl-c received, stopping");
        }
        _=handle => {
            log::error!("Syncing stopped");
        }
    }

    log::info!("Stopped");
}

pub fn load_plugins(client: &Client, settings_folder: impl AsRef<Path>) -> Result<()> {
    std::fs::create_dir_all(&settings_folder)?;
    #[cfg(feature = "yande_popular")]
    {
        let client = client.clone();
        let settings_folder = settings_folder.as_ref().to_path_buf();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move { yande_popular::run(client, settings_folder).await })
                .unwrap_or_else(|e| {
                    log::error!("yande_popular stop: {}", e);
                });
        });
    };

    #[cfg(feature = "webhook")]
    {
        let client = client.clone();
        let settings_folder = settings_folder.as_ref().to_path_buf();
        tokio::spawn(async move {
            webhook::run(client, settings_folder)
                .await
                .unwrap_or_else(|e| {
                    log::error!("webhook stop: {}", e);
                });
        });
    };

    #[cfg(feature = "qbittorrent")]
    {
        let client = client.clone();
        let settings_folder = settings_folder.as_ref().to_path_buf();
        tokio::spawn(async move {
            qbittorrent::run(client, settings_folder)
                .await
                .unwrap_or_else(|e| {
                    log::error!("qbittorrent stop: {}", e);
                });
        });
    };
    Ok(())
}
