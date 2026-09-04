use anyhow::Context;
use std::{fs, path::Path, process::Command};

pub fn embed(
    videos_path: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    /// Hardcoded by the littlefs component: CONFIG_LITTLEFS_BLOCK_SIZE = 4096.
    const LITTLEFS_BLOCK_SIZE_BYTES: u64 = 4096;
    /// littlefs partition size (partitions.csv); the image must not exceed this.
    const LITTLEFS_PARTITION_SIZE: u64 = 0xCF0000;

    let littlefs_path = workspace_root.as_ref().join("target/littlefs");
    let _ = fs::remove_dir_all(&littlefs_path);
    fs::create_dir_all(&littlefs_path)?;

    #[cfg(unix)]
    let symlink = std::os::unix::fs::symlink;
    #[cfg(windows)]
    let symlink = std::os::windows::fs::symlink_dir;

    symlink(
        workspace_root.as_ref().join("assets"),
        littlefs_path.join("assets"),
    )?;
    symlink(
        videos_path.as_ref().canonicalize()?,
        littlefs_path.join("videos"),
    )?;

    let status = Command::new("/bin/uvx")
        .current_dir(workspace_root.as_ref())
        .args([
            "littlefs-python@0.18.0",
            "create",
            littlefs_path
                .to_str()
                .ok_or(anyhow::anyhow!("Invalid video path"))?,
            "target/littlefs.bin",
            "-v",
            "--name-max",
            "64", // Must match CONFIG_LITTLEFS_OBJ_NAME_LEN (default 64) in the littlefs Kconfig.
            &format!(
                "--block-count={}",
                LITTLEFS_PARTITION_SIZE / LITTLEFS_BLOCK_SIZE_BYTES
            ),
            "--block-size",
            &LITTLEFS_BLOCK_SIZE_BYTES.to_string(),
            "--compact",
            "--no-pad",
        ])
        .status()
        .context("littlefs failed")?;

    if !status.success() {
        return Err(anyhow::anyhow!("littlefs failed"));
    }
    Ok(())
}
