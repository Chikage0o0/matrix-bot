use std::{collections::HashMap, path::Path};

use anyhow::{Ok, Result};
use once_cell::sync::Lazy;
use qbit_rs::{
    model::{Credential, GetTorrentListArg, State},
    Qbit,
};
use regex::Regex;

use crate::{upload, ROOM_MAP};

static MAGNET_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(magnet:[\?xt\=\w\:\&\;\+\%.]+)").unwrap());

pub async fn login(user: &str, pass: &str, url: &str) -> Result<Qbit> {
    let credential = Credential::new(user, pass);
    let api = Qbit::new(url, credential);
    api.login(false).await?;

    Ok(api)
}

// 如果是磁力链接，直接添加，并且如果连接中有名称，成功添加后返回名称
pub async fn add_torrent(
    api: &Qbit,
    magnet: &str,
    save_path: impl AsRef<Path>,
    room: &str,
    event: &str,
) -> Result<()> {
    if !MAGNET_REGEX.is_match(magnet) {
        return Err(anyhow::anyhow!("invalid magnet url"));
    }

    let url = url::Url::parse(magnet)?;
    let xt = url
        .query_pairs()
        .find(|(k, _)| k == "xt")
        .map(|(_, v)| v.to_string().to_ascii_lowercase());

    // 检测是否已经添加过
    let torrents = api.get_torrent_list(GetTorrentListArg::default()).await?;
    for torrent in torrents {
        if let Some(torrent_url) = torrent.magnet_uri {
            let torrent_url = url::Url::parse(&torrent_url)?;
            let torrent_xt = torrent_url
                .query_pairs()
                .find(|(k, _)| k == "xt")
                .map(|(_, v)| v.to_string().to_ascii_lowercase());
            if xt == torrent_xt {
                return Err(anyhow::anyhow!("torrent already exists"));
            }
        }
    }

    let url = qbit_rs::model::TorrentSource::Urls {
        urls: vec![url].into(),
    };

    let save_path = if save_path.as_ref().is_absolute() {
        save_path.as_ref().to_string_lossy().to_string()
    } else {
        std::env::current_dir()?
            .join(save_path.as_ref())
            .to_string_lossy()
            .to_string()
    };

    let arg = qbit_rs::model::AddTorrentArg {
        source: url,
        savepath: Some(save_path),
        tags: Some(event.to_string()),
        category: Some(room.to_string()),
        ..Default::default()
    };

    api.add_torrent(arg).await?;

    Ok(())
}

type ExpireTorrents = HashMap<String, qbit_rs::model::Torrent>;
type UploadTorrents = HashMap<String, qbit_rs::model::Torrent>;
pub async fn scan_torrent(api: &Qbit) -> Result<(ExpireTorrents, UploadTorrents)> {
    let torrents = api.get_torrent_list(GetTorrentListArg::default()).await?;
    let now_timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let mut expire_torrents = HashMap::new();
    let mut upload_torrents = HashMap::new();
    for torrent in torrents {
        if now_timestamp as i64 - torrent.added_on.unwrap_or(0) > 60 * 60 * 24 * 7
            && !matches!(
                &torrent.state,
                Some(qbit_rs::model::State::PausedUP)
                    | Some(qbit_rs::model::State::StalledUP)
                    | Some(qbit_rs::model::State::Uploading)
            )
            && torrent.hash.is_some()
            && torrent
                .category
                .as_ref()
                .is_some_and(|c| c.starts_with('!'))
        {
            expire_torrents.insert(torrent.hash.as_ref().unwrap().clone(), torrent.clone());
        }

        if matches!(
            &torrent.state,
            Some(qbit_rs::model::State::PausedUP)
                | Some(qbit_rs::model::State::StalledUP)
                | Some(qbit_rs::model::State::Uploading)
        ) && torrent
            .category
            .as_ref()
            .is_some_and(|c| c.starts_with('!'))
        {
            if let Some(hash) = &torrent.hash {
                upload_torrents.insert(hash.clone(), torrent.clone());
            }
        }
    }

    Ok((expire_torrents, upload_torrents))
}

pub async fn expire_torrents(api: &Qbit, torrents: &ExpireTorrents) -> Result<()> {
    for (hash, torrent) in torrents {
        let event_id = extract_non_empty_str(&torrent.tags);
        let room_id = extract_non_empty_str(&torrent.category);

        api.delete_torrents(vec![hash.clone()], true).await?;
        if room_id.is_some() {
            let room_id = room_id.unwrap();

            let room = ROOM_MAP.get().unwrap().get(room_id);

            if let Some((room, _)) = room {
                if let Some(name) = torrent.name.as_ref() {
                    let msg = format!("文件名：{}  \n下载超时，已删除。", name);
                    match event_id {
                        Some(event_id) => {
                            room.send_relates_msg(&msg, event_id, true).await?;
                            api.delete_tags(vec![event_id.clone()]).await?;
                        }
                        None => {
                            room.send_msg(&msg, true).await?;
                        }
                    }
                } else {
                    let msg = "下载超时，已删除。".to_string();
                    match event_id {
                        Some(event_id) => {
                            room.send_relates_msg(&msg, event_id, false).await?;
                            api.delete_tags(vec![event_id.clone()]).await?;
                        }
                        None => {
                            room.send_msg(&msg, false).await?;
                        }
                    }
                }
            }
        }
        log::info!("delete torrent: {}", &hash);
    }

    Ok(())
}

pub async fn upload_torrents(api: &Qbit, torrents: &UploadTorrents) -> Result<()> {
    for (hash, torrent) in torrents {
        log::info!("upload torrent: {}", &hash);
        let event_id = extract_non_empty_str(&torrent.tags);
        let room_id = extract_non_empty_str(&torrent.category);

        let file_path = torrent.content_path.as_ref().unwrap().clone();
        let file_path = std::path::Path::new(&file_path);

        let download_page = upload::gofile::upload(file_path).await?;
        if room_id.is_some() {
            let room_id = room_id.unwrap();

            let room = ROOM_MAP.get().unwrap().get(room_id);

            if let Some((room, _)) = room {
                if let Some(name) = torrent.name.as_ref() {
                    let msg = format!(
                        "文件名：{}  \n下载完成，[点击下载]({})。",
                        name, download_page
                    );
                    match event_id {
                        Some(event_id) => {
                            room.send_relates_msg(&msg, event_id, true).await?;
                            api.delete_tags(vec![event_id.clone()]).await?;
                        }
                        None => {
                            room.send_msg(&msg, true).await?;
                        }
                    }
                } else {
                    let msg = format!("下载完成，[点击下载]({})。", download_page);
                    match event_id {
                        Some(event_id) => {
                            room.send_relates_msg(&msg, event_id, false).await?;
                            api.delete_tags(vec![event_id.clone()]).await?;
                        }
                        None => {
                            room.send_msg(&msg, false).await?;
                        }
                    }
                }
            }
            api.delete_torrents(vec![hash.clone()], true).await?;
        }
    }

    Ok(())
}

pub async fn show_status(
    api: &Qbit,
    room_id: &str,
    event_id: Option<String>,
) -> Result<(String, String)> {
    let arg = GetTorrentListArg {
        category: Some(room_id.to_string()),
        tag: event_id,
        ..Default::default()
    };

    let torrents = api.get_torrent_list(arg).await?;
    let mut vec = Vec::new();
    for torrent in torrents {
        let name = torrent.name.unwrap_or_default();
        let state = match torrent.state {
            Some(State::PausedUP) => "下载完成",
            Some(State::StalledUP) => "下载完成",
            Some(State::Uploading) => "上传中",
            Some(State::PausedDL) => "暂停",
            Some(State::StalledDL) => "下载中(无连接)",
            Some(State::Downloading) => "下载中",
            Some(State::CheckingDL) => "检查中",
            Some(State::CheckingUP) => "检查中",
            Some(State::QueuedDL) => "等待下载",
            Some(State::QueuedUP) => "等待上传",
            Some(State::MetaDL) => "元数据下载中",
            Some(State::MissingFiles) => "缺少文件",
            Some(State::Unknown) => "未知",
            Some(State::Error) => "错误",
            Some(State::ForcedUP) => "强制上传",
            Some(State::ForcedDL) => "强制下载",
            Some(State::Allocating) => "分配空间",
            Some(State::CheckingResumeData) => "检查恢复数据",
            Some(State::Moving) => "移动中",
            None => "未知",
        };

        let progress = torrent.progress.unwrap_or_default();
        let progress = format!("{:.2}%", progress * 100.0);

        vec.push((name, state, progress));
    }

    let mut msg = String::new();
    for (name, state, progress) in &vec {
        msg.push_str(&format!("{} {} {}\n", name, state, progress));
    }

    let mut html_msg = String::from(
        "<table>
            <thead>
            <tr>
            <th>名称</th>
            <th>状态</th>
            <th>进度</th>
            </tr>
        </thead>
        <tbody>",
    );
    for (name, state, progress) in vec {
        html_msg.push_str(&format!(
            "<tr>
            <td>{}</td>
            <td>{}</td>
            <td>{}</td>
            </tr>",
            name, state, progress
        ));
    }
    html_msg.push_str("</tbody></table>");

    Ok((msg, html_msg))
}

fn extract_non_empty_str(s: &Option<String>) -> Option<&String> {
    if let Some(s) = s {
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}
