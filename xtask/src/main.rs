use anyhow::Context;
use fs_extra::dir::get_dir_content;
use glob::glob;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    let task = args.nth(1);
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();
    match task.as_deref() {
        Some("bmgr") => bmgr(&workspace_root)?,
        Some("flash") => flash(args, &workspace_root)?,
        Some("littlefs") => littlefs(args, &workspace_root)?,
        Some("encode") => encode(args, &workspace_root)?,
        Some("monitor") => monitor(&workspace_root)?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

bmgr
    generate esp_board_manager code in components/gen_bmgr_codes
flash firmware|littlefs
    flash release firmware or target/littlefs.bin
    (single espflash write-bin; set ESPFLASH_BAUD to override the baud)
littlefs <video-directory>
    build LittleFS filesystem image with embedded videos
encode interstitial <video-directory>
    generate interstitial.mp4 video in <video-directory>,
encode <video> <video-directory>
    encode <video> into <video-directory>
monitor
    monitor logs
"
    )
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

fn flash(mut args: env::Args, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    match args.next().as_deref() {
        Some("firmware") => flash_firmware(workspace_root),
        Some("littlefs") => flash_littlefs(workspace_root),
        _ => Err(anyhow::anyhow!("specify `firmware` or `littlefs`")),
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

fn flash_littlefs(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
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

fn littlefs(mut args: env::Args, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    /// Hardcoded by the littlefs component: CONFIG_LITTLEFS_BLOCK_SIZE = 4096.
    const LITTLEFS_BLOCK_SIZE_BYTES: u64 = 4096;
    /// littlefs partition size (partitions.csv); the image must not exceed this.
    const LITTLEFS_PARTITION_SIZE: u64 = 0xCF0000;
    /// LittleFS metadata reserved per file entry (entry struct + name + mtime attr).
    const LITTLEFS_ENTRY_SIZE: u64 = 128;

    let videos_path = args
        .next()
        .ok_or(anyhow::anyhow!("Missing path to a directory of videos"))?;
    let videos_path = Path::new(&videos_path);

    let dir_content = get_dir_content(videos_path).context("Failed to read video directory")?;
    let fs_size = dir_content.dir_size;
    let file_count = dir_content.files.len() as u64;

    // Image size = file data rounded to blocks + littlefs metadata overhead:
    // 2 blocks for the root directory metadata pair, 1 spare block for metadata
    // compaction during the image build, plus one entry (LITTLEFS_ENTRY_SIZE)
    // per file and one metadata pair (2 blocks) per subdirectory.
    let data_bytes = fs_size.div_ceil(LITTLEFS_BLOCK_SIZE_BYTES) * LITTLEFS_BLOCK_SIZE_BYTES;
    let entry_blocks = (file_count * LITTLEFS_ENTRY_SIZE).div_ceil(LITTLEFS_BLOCK_SIZE_BYTES);
    let dir_blocks = dir_content.directories.len() as u64 * 2;
    let overhead_blocks = 3 + entry_blocks + dir_blocks;
    let image_size = data_bytes + overhead_blocks * LITTLEFS_BLOCK_SIZE_BYTES;

    if image_size > LITTLEFS_PARTITION_SIZE {
        return Err(anyhow::anyhow!(
            "Videos need a {:#x}-byte image ({:#x} bytes of files + littlefs metadata), \
             which exceeds the {:#x}-byte littlefs partition. Remove content or enlarge the \
             partition in partitions.csv.",
            image_size,
            fs_size,
            LITTLEFS_PARTITION_SIZE
        ));
    }

    let littlefs_python = ensure_littlefs_python(workspace_root.as_ref())?;
    println!(
        "==> Building {}-byte LittleFS image for {} files ({:.2} MB) from {}...",
        image_size,
        file_count,
        fs_size as f64 / (1024.0 * 1024.0),
        videos_path.display()
    );

    let status = Command::new(littlefs_python)
        .current_dir(workspace_root.as_ref())
        .args([
            "create",
            videos_path
                .to_str()
                .ok_or(anyhow::anyhow!("Invalid video path"))?,
            "target/littlefs.bin",
            "-v",
            &format!("--fs-size=0x{:X}", image_size),
            "--name-max",
            "64", // Must match CONFIG_LITTLEFS_OBJ_NAME_LEN (default 64) in the littlefs Kconfig.
            "--block-size",
            &LITTLEFS_BLOCK_SIZE_BYTES.to_string(),
        ])
        .status()
        .context("littlefsgen failed")?;

    if !status.success() {
        return Err(anyhow::anyhow!("littlefsgen failed"));
    }
    Ok(())
}

fn encode(mut args: env::Args, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let command = args
        .next()
        .ok_or(anyhow::anyhow!("Missing `interstitial` or `<video>`"))?;
    match command.as_str() {
        "interstitial" => encode_interstitial(args, workspace_root),
        video_path => encode_video(video_path, args, workspace_root),
    }
}

fn encode_video(
    video_path: &str,
    mut args: env::Args,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let output_directory = args
        .next()
        .ok_or(anyhow::anyhow!("Missing output directory"))?;
    let mut output_path = PathBuf::from(video_path);
    output_path.set_extension("mp4");
    output_path = Path::new(&output_directory).join(output_path.file_name().ok_or(
        anyhow::anyhow!("Invalid video path {}", output_path.display()),
    )?);
    let status = Command::new("ffmpeg")
        .current_dir(workspace_root.as_ref())
        .args([
            "-i",
            video_path,
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
    mut args: env::Args,
    workspace_root: impl AsRef<Path>,
) -> anyhow::Result<()> {
    // Unique snow/static frames
    const UNIQUE_FRAMES: u32 = 4;
    // Total frames in video - we use "-refs 4" so there is very little overhead for additional frames
    const TOTAL_FRAMES: u32 = UNIQUE_FRAMES * 6;

    let output_directory = args
        .next()
        .ok_or(anyhow::anyhow!("Missing output directory"))?;
    let output_path = Path::new(&output_directory).join("interstitial.mp4");
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

/// Returns the path to the `littlefs-python` CLI, installing it into the
/// embuild-managed IDF virtualenv on first use (same tool the littlefs
/// component uses via its `littlefs_create_partition_image` CMake helper).
fn ensure_littlefs_python(workspace_root: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    let python_path = find_python_path(workspace_root.as_ref())?;
    let bin_dir = python_path
        .parent()
        .ok_or(anyhow::anyhow!("Failed to resolve Python bin directory"))?;
    let littlefs_python = if cfg!(windows) {
        bin_dir.join("littlefs-python.exe")
    } else {
        bin_dir.join("littlefs-python")
    };

    if !littlefs_python.exists() {
        println!("==> Installing littlefs-python into IDF virtualenv...");
        ensure_espidf_components(workspace_root.as_ref())?;
        let requirements = find_latest_espidf_path(
            workspace_root.as_ref(),
            "out/managed_components/joltwallet__littlefs/image-building-requirements.txt",
        )?;
        let status = Command::new(&python_path)
            .args([
                "-m",
                "pip",
                "install",
                "-r",
                requirements
                    .to_str()
                    .ok_or(anyhow::anyhow!("Invalid requirements path"))?,
            ])
            .status()
            .context("Failed to install littlefs-python")?;
        if !status.success() {
            return Err(anyhow::anyhow!("Failed to install littlefs-python"));
        }
    }
    Ok(littlefs_python)
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
