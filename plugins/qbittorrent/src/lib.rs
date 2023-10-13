use anyhow::Result;
use matrix_bot_core::matrix::client::Client;

mod qbit;
mod setting;

#[allow(unused_variables)]
pub async fn run(client: Client, plugin_folder: impl AsRef<std::path::Path>) -> Result<()> {
    log::info!("start yande_popular");

    let setting = setting::get_or_init(plugin_folder)?;

    if setting.use_internal_qbit {}

    Ok(())
}
