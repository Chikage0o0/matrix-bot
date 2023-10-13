use std::path::{Path, PathBuf};

use anyhow::Result;
use image_compressor::{
    compressor::{Compressor, ResizeType},
    Factor,
};

pub fn resize_and_compress(path: &Path, size: usize) -> Result<PathBuf> {
    let source = path;
    let dest = path.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dest)?;
    let mut comp = Compressor::new(source, &dest);
    comp.set_factor(Factor::new_with_resize_type(
        85.0,
        ResizeType::LongestSidePixels(size),
    ));
    comp.set_delete_source(false);
    comp.set_overwrite_dest(true);
    let path = comp.compress_to_jpg().unwrap_or_else(|e| {
        log::error!("compress failed: {}", e);
        source.to_path_buf()
    });
    Ok(path)
}
