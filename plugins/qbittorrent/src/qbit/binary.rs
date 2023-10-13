use anyhow::Result;

use std::{
    path::Path,
    process::{Child, Stdio},
};

#[allow(dead_code)]
fn get_download_link() -> Result<String> {
    if cfg!(target_os = "linux") != true {
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
#[allow(dead_code)]
fn download_binary(path: impl AsRef<Path>) -> Result<()> {
    let link = get_download_link()?;
    let resp = reqwest::blocking::get(link)?;
    let binary = resp.bytes()?;
    std::fs::create_dir_all(path.as_ref().parent().unwrap())?;
    std::fs::write(&path, binary)?;
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, PermissionsExt::from_mode(0o755))?;
    }
    Ok(())
}
#[allow(dead_code)]
pub fn run(runtime_folder: impl AsRef<Path>, port: u16) -> Result<Child> {
    let binary_path = runtime_folder.as_ref().join("qbittorrent-nox");
    if !binary_path.exists() {
        download_binary(&binary_path)?;
    }

    let qbittorrent_folder = runtime_folder.as_ref().join("config");
    let download_folder = runtime_folder.as_ref().join("download");
    if !download_folder.exists() {
        std::fs::create_dir_all(&download_folder)?;
    }

    let qbitconfig = qbittorrent_folder
        .join("qBittorrent")
        .join("config")
        .join("qBittorrent.conf");
    if !qbitconfig.exists() {
        std::fs::create_dir_all(qbitconfig.parent().unwrap())?;
        std::fs::write(
            &qbitconfig,
            format!(
                r#"[LegalNotice]
                    Accepted=true

                    [BitTorrent]
                    Session\DefaultSavePath={}
                "#,
                download_folder.to_string_lossy()
            ),
        )?;
    }

    let mut cmd = std::process::Command::new(binary_path);
    cmd.arg(format!("--webui-port={}", port)).arg(format!(
        "--profile={}",
        qbittorrent_folder.to_string_lossy()
    ));
    cmd.stdout(Stdio::null());

    let child = cmd.spawn()?;

    Ok(child)
}

#[cfg(test)]
mod test {

    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_running() {
        let runtime_folder = PathBuf::from("/tmp/qbit");
        let mut child = run(&runtime_folder, 8080).unwrap();
        std::thread::sleep(std::time::Duration::from_secs(10));

        // is still running
        assert!(child.try_wait().unwrap().is_none());
        let logs = runtime_folder
            .join("config")
            .join("qBittorrent")
            .join("data")
            .join("logs")
            .join("qbittorrent.log");

        if let Ok(logs) = std::fs::read_to_string(logs) {
            assert!(logs.contains("Web UI: Now listening on IP"));
        } else {
            panic!("no logs");
        }

        child.kill().unwrap();
        std::fs::remove_dir_all("/tmp/qbit").unwrap();
    }
}
