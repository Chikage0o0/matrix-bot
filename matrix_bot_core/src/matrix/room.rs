use anyhow::{anyhow, Result};
use image::GenericImageView;
use matrix_sdk::ruma::events::room::message::RoomMessageEvent;
use matrix_sdk::ruma::OwnedEventId;
use matrix_sdk::{
    attachment::AttachmentConfig, room::Joined,
    ruma::events::room::message::RoomMessageEventContent,
};
use matrix_sdk::{config::SyncSettings, Client};
use mime_guess::mime;

use std::{convert::TryInto, fs, path::Path};

#[derive(Debug, Clone)]
pub struct Room(pub Joined);

impl Room {
    pub async fn new(client: &Client, room_id: &str) -> Result<Self> {
        if !client.logged_in() {
            return Err(anyhow!("Not logged in"));
        }
        client
            .sync_once(SyncSettings::new())
            .await
            .map_err(|e| anyhow!("Can't sync: {}", e))?;
        let room = client.get_joined_room(room_id.try_into()?);

        if let Some(room) = room {
            Ok(Room(room))
        } else {
            if let Some(room) = client.get_invited_room(room_id.try_into()?) {
                room.accept_invitation()
                    .await
                    .map_err(|e| anyhow!("Can't accept invitation:{e}"))?;
                let room = client
                    .get_joined_room(room_id.try_into()?)
                    .ok_or(anyhow!("Can't find room {}", room_id))?;
                Ok(Room(room))
            } else {
                Err(anyhow!("Can't find room {}", room_id))
            }
        }
    }

    pub async fn send_attachment(&self, file_path: impl AsRef<Path>) -> Result<()> {
        let (filename, mime, file, config) = Self::prepare_send_attachment(file_path)?;
        self.0
            .send_attachment(&filename, &mime, &file, config)
            .await?;
        Ok(())
    }

    fn prepare_send_attachment<'a>(
        file_path: impl AsRef<Path>,
    ) -> Result<(String, mime_guess::Mime, Vec<u8>, AttachmentConfig<'a>)> {
        let file = fs::read(&file_path)?;
        let filename = file_path
            .as_ref()
            .file_name()
            .unwrap_or(std::ffi::OsStr::new("image.jpg"))
            .to_str()
            .unwrap_or("image.jpg");
        let mime = mime_guess::from_path(&file_path).first_or_octet_stream();

        let config = match mime.type_() {
            mime::IMAGE => {
                // 从文件Bytes获取图片信息
                let image = image::load_from_memory(&file)?;
                let (width, height) = image.dimensions();
                let blurhash = blurhash::encode(4, 3, width, height, image.to_rgba8().as_raw())?;

                let info = matrix_sdk::attachment::BaseImageInfo {
                    height: Some(height.try_into()?),
                    width: Some(width.try_into()?),
                    size: Some(file.len().try_into()?),
                    blurhash: Some(blurhash),
                };

                AttachmentConfig::new().info(matrix_sdk::attachment::AttachmentInfo::Image(info))
            }
            _ => AttachmentConfig::default(),
        };

        Ok((filename.to_string(), mime, file, config))
    }

    pub async fn send_msg(&self, msg: &str, is_markdown: bool) -> Result<()> {
        let msg = if is_markdown {
            RoomMessageEventContent::text_markdown(msg)
        } else {
            RoomMessageEventContent::text_plain(msg)
        };
        self.0.send(msg, None).await?;

        Ok(())
    }

    pub async fn send_relates_msg(
        &self,
        msg: &str,
        event_id: &OwnedEventId,
        is_markdown: bool,
    ) -> Result<()> {
        let msg = if is_markdown {
            RoomMessageEventContent::text_markdown(msg)
        } else {
            RoomMessageEventContent::text_plain(msg)
        };
        let timeline_event = self.0.event(event_id).await?;
        let event_content = timeline_event.event.deserialize_as::<RoomMessageEvent>()?;
        let original_message = event_content.as_original().unwrap();
        let msg = msg.make_reply_to(original_message);

        self.0.send(msg, None).await?;

        Ok(())
    }
}
