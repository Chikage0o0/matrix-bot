use std::{fs, ops::Deref, path::Path};

use anyhow::Result;
use matrix_sdk::{self, config::SyncSettings};

use url::Url;

#[derive(Debug, Clone)]
pub struct Client(pub matrix_sdk::Client);

unsafe impl Send for Client {}
unsafe impl Sync for Client {}

impl Deref for Client {
    type Target = matrix_sdk::Client;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Client {
    pub async fn login(
        homeserver_url: &str,
        username: &str,
        password: &str,
        session_file: impl AsRef<Path>,
        db_path: impl AsRef<Path>,
    ) -> Result<Client> {
        let homeserver_url = Url::parse(homeserver_url).expect("Couldn't parse the homeserver URL");

        std::fs::create_dir_all(&db_path)?;
        std::fs::create_dir_all(&session_file.as_ref().parent().unwrap())?;

        let mut client = matrix_sdk::Client::builder()
            .homeserver_url(&homeserver_url)
            .sled_store(&db_path, None)?
            .build()
            .await
            .map_err(|e| {
                log::error!("client build error: {}", e);
                e
            })?;

        if !client.logged_in() {
            if session_file.as_ref().exists()
                && Self::restore_login(&client, &session_file).await.is_ok()
                && client.logged_in()
                && client.sync_once(SyncSettings::new()).await.is_ok()
            {
                log::info!("Restored login from session file");
            } else {
                drop(client);
                // 清理数据库
                fs::remove_dir_all(&db_path)?;
                client = matrix_sdk::Client::builder()
                    .homeserver_url(&homeserver_url)
                    .sled_store(db_path, None)?
                    .build()
                    .await
                    .map_err(|e| {
                        log::error!("client build error: {}", e);
                        e
                    })?;
                Self::login_username(&client, &session_file, username, password).await?;
                client.sync_once(SyncSettings::new()).await?;
            };

            log::info!("Logged in as {}", username);
        }

        Ok(Client(client))
    }

    async fn restore_login(
        client: &matrix_sdk::Client,
        session_file: impl AsRef<Path>,
    ) -> Result<()> {
        let session = fs::read_to_string(session_file)?;
        let session = serde_json::from_str(&session)?;
        client.restore_login(session).await.map_err(|e| {
            log::error!("restore login error: {}", e);
            e
        })?;
        Ok(())
    }

    async fn login_username(
        client: &matrix_sdk::Client,
        session_file: impl AsRef<Path>,
        username: &str,
        password: &str,
    ) -> Result<()> {
        client
            .login_username(username, password)
            .initial_device_display_name("matrix_bot")
            .send()
            .await?;
        let session = client.session();
        if let Some(session) = session {
            let session = serde_json::to_string(&session)?;
            fs::write(session_file, session)?;
        };
        Ok(())
    }
}
