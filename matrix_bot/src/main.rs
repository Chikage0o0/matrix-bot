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

    /// Plugin selection
    /// Available plugins: yande_popular, webhook, qbittorrent
    /// Example: -P yande_popular,webhook
    /// Example: -P all
    #[arg(short = 'P', long, env = "PLUGINS", default_value = "all")]
    plugins: String,
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

    let _ = load_plugins(&matrix_client, args.data.join("plugins"), &args.plugins);

    let ctrlc = tokio::signal::ctrl_c();
    let mut term= tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();

    let client = matrix_client.clone();

    let handle = tokio::spawn(async move {
        client
            .sync(SyncSettings::new().timeout(std::time::Duration::from_secs(30)))
            .await
    });

    #[cfg(unix)]
    tokio::select! {
        _=ctrlc => {
            log::info!("Ctrl-c received, stopping");
        }
        _=handle => {
            log::error!("Syncing stopped");
        }
    
        _=term.recv() => {
            log::info!("SIGTERM received, stopping");
        }
    }

    #[cfg(not(unix))]
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

pub fn load_plugins(
    client: &Client,
    settings_folder: impl AsRef<Path>,
    selection: &str,
) -> Result<()> {
    std::fs::create_dir_all(&settings_folder)?;

    let selection = selection
        .split(',')
        .map(|s| s.trim().to_string())
        .collect::<Vec<_>>();

    #[cfg(feature = "yande_popular")]
    {
        if selection.contains(&"yande_popular".to_string())
            || selection.contains(&"all".to_string())
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
        }
    };

    #[cfg(feature = "webhook")]
    {
        if selection.contains(&"webhook".to_string()) || selection.contains(&"all".to_string()) {
            let client = client.clone();
            let settings_folder = settings_folder.as_ref().to_path_buf();
            tokio::spawn(async move {
                webhook::run(client, settings_folder)
                    .await
                    .unwrap_or_else(|e| {
                        log::error!("webhook stop: {}", e);
                    });
            });
        }
    };

    #[cfg(feature = "qbittorrent")]
    {
        if selection.contains(&"qbittorrent".to_string()) || selection.contains(&"all".to_string())
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
        }
    };
    Ok(())
}
