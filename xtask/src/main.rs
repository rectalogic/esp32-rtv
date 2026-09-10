use anyhow::Context;
use clap::{Parser, Subcommand};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};
use xtask::bsky::{BskyArgs, bsky};
use xtask::{
    bmgr::bmgr,
    embed::embed,
    encode::{EncodeArgs, encode},
    flash::{FlashType, flash},
};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Xtask {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Regenerate esp_board_manager code in components/gen_bmgr_codes
    Bmgr,
    /// Flash firmware or embedded video filesystem
    Flash {
        #[arg(value_enum)]
        flash_type: FlashType,
    },
    /// Build embedded video filesystem
    Embed {
        /// Directory of videos to embed
        video_dir: PathBuf,
    },
    /// Encode video or interstitial
    Encode(EncodeArgs),
    /// Monitor device logs
    Monitor,
    Bsky(BskyArgs),
}

fn main() -> anyhow::Result<()> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();

    let xtask = Xtask::parse();
    match &xtask.command {
        Commands::Bmgr => bmgr(&workspace_root),
        &Commands::Flash { flash_type } => flash(flash_type, &workspace_root),
        Commands::Embed { video_dir } => embed(video_dir, &workspace_root),
        Commands::Encode(encode_args) => encode(encode_args, &workspace_root),
        Commands::Monitor => monitor(&workspace_root),
        Commands::Bsky(bsky_args) => bsky(bsky_args),
    }
}

fn monitor(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    Command::new("espflash")
        .current_dir(workspace_root.as_ref())
        .arg("monitor")
        .status()
        .context("`espflash` monitor failed")?;
    Ok(())
}
