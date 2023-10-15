use matrix_bot_core::{
    matrix,
    matrix_sdk::{
        self,
        ruma::events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, Relation, TextMessageEventContent,
        },
    },
};

use crate::{
    qbit::ops::{add_torrent, show_status},
    API, ROOM_MAP,
};

pub fn add_listener(room: &matrix::room::Room) {
    room.0.add_event_handler(
        |event: OriginalSyncRoomMessageEvent, room: matrix_sdk::room::Room| async move {
            if let matrix_sdk::room::Room::Joined(room) = room {
                let msg_body = match event.content.msgtype {
                    MessageType::Text(TextMessageEventContent { body, .. }) => body,
                    _ => return,
                };

                let msg_body = msg_body.trim();

                if msg_body.is_empty() {
                    return;
                }
                if msg_body.starts_with("!download") {
                    let link = msg_body
                        .trim_start_matches("!download")
                        .trim()
                        .split_ascii_whitespace()
                        .next()
                        .unwrap_or_default();

                    let room_id = room.room_id().as_str();

                    let map = ROOM_MAP.get().and_then(|map| map.get(room_id));

                    if let Some((room, setting)) = map {
                        let result = add_torrent(
                            API.get().unwrap(),
                            link,
                            setting.download_path.as_path(),
                            room_id,
                            event.event_id.as_str(),
                        )
                        .await;

                        match result {
                            Ok(_) => {
                                room.send_relates_msg("添加成功", event.event_id.as_str(), false)
                                    .await
                                    .unwrap_or_else(|e| {
                                        log::error!("send message failed: {}", e);
                                    });
                            }
                            Err(e) => {
                                room.send_relates_msg(
                                    &format!("添加失败: {}", e),
                                    event.event_id.as_str(),
                                    false,
                                )
                                .await
                                .unwrap_or_else(|e| {
                                    log::error!("send message failed: {}", e);
                                });
                            }
                        }
                    }
                }
            }
        },
    );

    room.0.add_event_handler(
        |event: OriginalSyncRoomMessageEvent, room: matrix_sdk::room::Room| async move {
            if let matrix_sdk::room::Room::Joined(room) = room {
                let msg_body = match event.content.msgtype {
                    MessageType::Text(TextMessageEventContent { body, .. }) => body,
                    _ => return,
                };

                let msg_body = msg_body.trim();

                if msg_body.is_empty() {
                    return;
                }
                if msg_body.starts_with("!qbithelp") {
                    let room_id = room.room_id().as_str();

                    let map = ROOM_MAP.get().and_then(|map| map.get(room_id));

                    if let Some((room, _setting)) = map {
                        let msg = "!download <magnet_url> - 添加磁力至下载\n!status - 查看下载状态";
                        room.send_relates_msg(&msg, event.event_id.as_str(), false)
                            .await
                            .unwrap_or_else(|e| {
                                log::error!("send message failed: {}", e);
                            });
                    }
                }
            }
        },
    );

    room.0.add_event_handler(
        |event: OriginalSyncRoomMessageEvent, room: matrix_sdk::room::Room| async move {
            if let matrix_sdk::room::Room::Joined(room) = room {
                let msg_body = match event.content.msgtype {
                    MessageType::Text(TextMessageEventContent { body, .. }) => body,
                    _ => return,
                };

                let msg_body = msg_body.trim();

                if msg_body.is_empty() {
                    return;
                }
                if msg_body.starts_with("!status") {
                    let room_id = room.room_id().as_str();

                    let map = ROOM_MAP.get().and_then(|map| map.get(room_id));

                    let reply_event_id = event.content.relates_to.as_ref().and_then(|r| {
                        if let Relation::Reply { in_reply_to } = r {
                            let event_id = in_reply_to.event_id.as_str();
                            Some(event_id.to_string())
                        } else {
                            None
                        }
                    });

                    if let Some((room, _setting)) = map {
                        let msg = show_status(API.get().unwrap(), room_id, reply_event_id).await;
                        match msg {
                            Ok((msg, html_msg)) => {
                                room.send_relates_html(&msg, &html_msg, event.event_id.as_str())
                                    .await
                                    .unwrap_or_else(|e| {
                                        log::error!("send message failed: {}", e);
                                    });
                            }
                            Err(e) => {
                                room.send_relates_msg(
                                    &format!("获取失败: {}", e),
                                    event.event_id.as_str(),
                                    false,
                                )
                                .await
                                .unwrap_or_else(|e| {
                                    log::error!("send message failed: {}", e);
                                });
                            }
                        }
                    }
                }
            }
        },
    );
}
