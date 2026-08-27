use anyhow::Context;
use clap::{Args, Parser, Subcommand, ValueEnum};
use glob::glob;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
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
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum FlashType {
    Firmware,
    Embed,
}

#[derive(Args)]
#[command(flatten_help = true)]
struct EncodeArgs {
    #[command(subcommand)]
    command: EncodeCommands,
    /// Video output directory
    output_directory: PathBuf,
}

#[derive(Subcommand)]
enum EncodeCommands {
    /// Encode interstitial.mp4 video
    Interstitial,
    /// Encode video
    Video {
        /// Source video to encode
        video_path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();

    let xtask = Xtask::parse();
    match &xtask.command {
        Commands::Bmgr => bmgr(&workspace_root),
        Commands::Flash { flash_type } => match flash_type {
            FlashType::Firmware => flash_firmware(&workspace_root),
            FlashType::Embed => flash_embed(&workspace_root),
        },
        Commands::Embed { video_dir } => embed(video_dir, &workspace_root),
        Commands::Encode(EncodeArgs {
            command,
            output_directory,
        }) => match command {
            EncodeCommands::Interstitial => encode_interstitial(output_directory, &workspace_root),
            EncodeCommands::Video { video_path } => {
                encode_video(video_path, output_directory, &workspace_root)
            }
        },
        Commands::Monitor => monitor(&workspace_root),
    }
}

fn bmgr(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    ensure_espidf_components(workspace_root.as_ref())?;
    let python_path = find_python_path(workspace_root.as_ref())?;

    let bmgr_script = find_latest_espidf_path(
        workspace_root.as_ref(),
        "out/managed_components/espressif__esp_board_manager/gen_bmgr_config_codes.py",
    )?;
    println!("==> Found BMGR script at: {}", bmgr_script.display());

    println!("==> Generating BMGR C code...");
    let status = Command::new(python_path)
        .current_dir(&workspace_root)
        .arg(bmgr_script)
        .arg("--project-dir")
        .arg(workspace_root.as_ref())
        .arg("-b")
        .arg("e32c28p") // custom board name
        .status()
        .context("Failed to execute BMGR generator")?;

    if !status.success() {
        return Err(anyhow::anyhow!("BMGR code generation failed"));
    }

    Ok(())
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
            "0x310000", // Must match partitions.csv littlefs partition offset, view with "espflash partition-table partitions.csv"
            "target/littlefs.bin",
        ])
        .status()
        .context("`espflash` littlefs failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`espflash write-bin` failed for littlefs"));
    }
    Ok(())
}

fn embed(videos_path: impl AsRef<Path>, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
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

fn encode_video(
    video_path: impl AsRef<Path>,
    output_directory: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let video_path = video_path.as_ref();
    let mut output_path = PathBuf::from(video_path);
    output_path.set_extension("mp4");
    output_path = Path::new(output_directory.as_ref()).join(output_path.file_name().ok_or(
        anyhow::anyhow!("Invalid video path {}", output_path.display()),
    )?);
    let status = Command::new("ffmpeg")
        .current_dir(workspace_root.as_ref())
        .args([
            "-i",
            video_path.to_str().ok_or(anyhow::anyhow!("Invalid video path {}", video_path.display()))?,
            "-r", "15",
            "-c:v", "libx264",
            "-preset", "veryslow",
            "-profile:v", "baseline",
            "-level", "3.0",
            "-c:a", "aac",
            "-ar", "16000",
            "-ac", "1",
            "-vf", "scale=320x240:force_original_aspect_ratio=decrease:reset_sar=1:flags=lanczos,pad=320:240:(ow-iw)/2:(oh-ih)/2,format=yuv420p",
            "-y",
            output_path.to_str().ok_or(anyhow::anyhow!("Invalid output path {}", output_path.display()))?,
        ])
        .status()
        .context("`ffmpeg` failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`ffmpeg` failed"));
    }
    Ok(())
}

fn encode_interstitial(
    output_directory: impl AsRef<Path>,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    // Unique snow/static frames
    const UNIQUE_FRAMES: u32 = 4;
    // Total frames in video - we use "-refs 4" so there is very little overhead for additional frames
    const TOTAL_FRAMES: u32 = UNIQUE_FRAMES * 6;

    let output_path = Path::new(output_directory.as_ref()).join("interstitial.mp4");
    let status = Command::new("ffmpeg")
        .current_dir(workspace_root.as_ref())
        .args([
            "-f", "lavfi",
            "-i", &format!("color=c=0x808080:s=320x240:r=15,noise=alls=100:allf=t+u,eq=contrast=1.4,format=yuv420p,trim=end_frame={UNIQUE_FRAMES},setpts=PTS-STARTPTS,loop=loop=-1:size={UNIQUE_FRAMES}:start=0"),
            "-frames:v", &TOTAL_FRAMES.to_string(),
            "-c:v", "libx264",
            "-preset", "veryslow",
            "-profile:v", "baseline",
            "-level", "3.0",
            "-bf", "0",
            "-refs", &UNIQUE_FRAMES.to_string(),
            "-sc_threshold", "0",
            "-x264-params", "scenecut=0",
            "-crf", "23",
            "-pix_fmt", "yuv420p",
            "-y", output_path.to_str().ok_or(anyhow::anyhow!("Invalid output path {}", output_path.display()))?,
        ])
        .status()
        .context("`ffmpeg` failed")?;
    if !status.success() {
        return Err(anyhow::anyhow!("`ffmpeg` failed"));
    }
    Ok(())
}

fn monitor(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    Command::new("espflash")
        .current_dir(workspace_root.as_ref())
        .arg("monitor")
        .status()
        .context("`espflash` monitor failed")?;
    Ok(())
}

fn find_python_path(workspace_root: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let python_glob = if cfg!(windows) {
        workspace_root
            .as_ref()
            .join(".embuild/espressif/python_env/idf*_py*_env/Scripts/python.exe")
    } else {
        workspace_root
            .as_ref()
            .join(".embuild/espressif/python_env/idf*_py*_env/bin/python")
    };
    let python_path = glob(
        python_glob
            .to_str()
            .ok_or(anyhow::anyhow!("Python glob failed"))?,
    )
    .expect("Failed to read glob pattern")
    .filter_map(Result::ok)
    .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok())
    .ok_or(anyhow::anyhow!(
        "Could not find embuild-managed Python environment."
    ))?;
    Ok(python_path)
}

fn ensure_espidf_components(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    println!("==> Initializing ESP-IDF environment and fetching components...");
    Command::new("cargo")
        .current_dir(workspace_root.as_ref())
        .args(["check", "--release"])
        .status()
        .context("`cargo check` failed")?;
    Ok(())
}

fn find_latest_espidf_path(
    workspace_root: impl AsRef<Path>,
    path: &str,
) -> anyhow::Result<PathBuf> {
    let path_glob = workspace_root
        .as_ref()
        .join("target/xtensa-esp32s3-espidf/release/build/esp-idf-sys-*")
        .join(path);

    let mut paths: Vec<_> = glob(
        path_glob
            .to_str()
            .ok_or(anyhow::anyhow!("Failed to read glob pattern"))?,
    )
    .context("Failed to read glob pattern")?
    .filter_map(Result::ok)
    .collect();

    if paths.is_empty() {
        return Err(anyhow::anyhow!("Path {path} not found in esp-idf-sys"));
    }

    paths.sort_by(|a, b| {
        let time_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let time_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    let resolved_path = paths.swap_remove(0);
    println!("==> Found path at: {}", resolved_path.display());
    Ok(resolved_path)
}
