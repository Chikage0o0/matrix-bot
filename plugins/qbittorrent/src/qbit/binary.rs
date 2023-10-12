use anyhow::Result;

#[allow(dead_code)]
fn get_download_link() -> Result<String> {
    if !(cfg!(target_os = "linux")) {
        return Err(anyhow::anyhow!("only support linux"));
    }

    let mut link =
        "https://github.com/userdocs/qbittorrent-nox-static/releases/latest/download/".to_string();
    if cfg!(target_arch = "x86_64") {
        link.push_str("x86_64-qbittorrent-nox");
    } else if cfg!(target_arch = "aarch64") {
        link.push_str("aarch64-qbittorrent-nox");
    } else if cfg!(target_arch = "x86") {
        link.push_str("x86-qbittorrent-nox");
    } else {
        return Err(anyhow::anyhow!("only support x86_64 and aarch64"));
    }
    Ok(link)
}
