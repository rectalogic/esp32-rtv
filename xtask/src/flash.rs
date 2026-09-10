use anyhow::Context;
use clap::ValueEnum;
use std::{path::Path, process::Command};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum FlashType {
    Firmware,
    Embed,
}

pub fn flash(flash_type: FlashType, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    match flash_type {
        FlashType::Firmware => flash_firmware(&workspace_root),
        FlashType::Embed => flash_embed(&workspace_root),
    }
}

fn flash_firmware(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let status = Command::new("espflash")
        .current_dir(workspace_root.as_ref())
        .args([
            "flash",
            "--monitor",
            "--chip",
            "esp32s3",
            "target/xtensa-esp32s3-espidf/release/esp32-rtv",
        ])
        .status()
        .context("`espflash` firmware failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`espflash` firmware failed"));
    }
    Ok(())
}

fn flash_embed(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let status = Command::new("espflash")
        .current_dir(workspace_root.as_ref())
        .args([
            "write-bin",
            "--chip",
            "esp32s3",
            "0x410000", // Must match partitions.csv littlefs partition offset, view with "espflash partition-table partitions.csv"
            "target/littlefs.bin",
        ])
        .status()
        .context("`espflash` littlefs failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`espflash write-bin` failed for littlefs"));
    }
    Ok(())
}
