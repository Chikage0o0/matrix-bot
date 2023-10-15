use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, path::Path};
use walkdir::WalkDir;

#[derive(Debug, Deserialize, Serialize)]
struct ServerResponse {
    status: String,
    data: Server,
}

#[derive(Debug, Deserialize, Serialize)]
struct Server {
    server: String,
}

pub async fn upload<P: AsRef<Path>>(file_path: P) -> Result<String> {
    // curl https://api.gofile.io/getServer
    // {"status":"ok","data":{"server":"srv-store3"}}
    let resp = reqwest::get("https://api.gofile.io/getServer").await?;
    let resp = resp.json::<ServerResponse>().await?;
    let status = resp.status.as_str();
    if status != "ok" {
        return Err(anyhow::anyhow!("Failed to get server:{:?}", resp));
    }
    let server = resp.data.server;

    if file_path.as_ref().is_dir() {
        let mut token = None;
        let mut folder_id = None;
        let mut download_page = String::new();
        for entry in WalkDir::new(&file_path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().is_file()
                    && !e.file_name().to_string_lossy().to_string().starts_with('.')
            })
        {
            let file_name = entry.file_name().to_string_lossy().to_string();
            let file = std::fs::read(entry.path())?;
            let upload = UploadParams {
                server: server.clone(),
                token: token.clone(),
                folder_id: folder_id.clone(),
                file,
                file_name,
            };
            let upload = upload_file(upload).await?;
            if upload.guest_token.is_some() {
                token = upload.guest_token;
            }
            if folder_id.is_none() {
                folder_id = Some(upload.parent_folder);
            }
            download_page = upload.download_page;
        }
        if download_page.is_empty() {
            return Err(anyhow::anyhow!(
                "Failed to upload file:{:?}",
                file_path.as_ref()
            ));
        }
        Ok(download_page)
    } else {
        let file_name = file_path
            .as_ref()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let file = std::fs::read(file_path)?;
        let upload = UploadParams {
            server,
            token: None,
            folder_id: None,
            file,
            file_name,
        };
        let upload = upload_file(upload).await?;
        Ok(upload.download_page)
    }
}

#[derive(Debug)]
struct UploadParams {
    server: String,
    token: Option<String>,
    folder_id: Option<String>,
    file: Vec<u8>,
    file_name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct UploadResponse {
    status: String,
    data: UploadInfo,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadInfo {
    guest_token: Option<String>,
    download_page: String,
    code: String,
    parent_folder: String,
    file_id: String,
    file_name: String,
    md5: String,
}
async fn upload_file(parameter: UploadParams) -> Result<UploadInfo> {
    // curl -F file=@someFile.txt https://store1.gofile.io/uploadFile
    let url = format!("https://{}.gofile.io/uploadFile", parameter.server);

    // 一定要有file_name方法，且参数不能为空，否则数据上传失败
    let part =
        reqwest::multipart::Part::bytes(Cow::from(parameter.file)).file_name(parameter.file_name);
    let mut form = reqwest::multipart::Form::new().part("file", part);
    if let Some(token) = parameter.token {
        form = form.text("token", token);
    }
    if let Some(folder_id) = parameter.folder_id {
        form = form.text("folderId", folder_id);
    }
    let client = reqwest::ClientBuilder::new().build()?;

    let resp = client.post(url).multipart(form).send().await?;
    let resp = resp.json::<UploadResponse>().await?;
    let status = resp.status.as_str();
    if status != "ok" {
        return Err(anyhow::anyhow!("Failed to upload file:{:?}", resp));
    }

    Ok(resp.data)
}
