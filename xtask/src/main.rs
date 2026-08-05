use anyhow::Context;
use glob::glob;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Must match CONFIG_LITTLEFS_OBJ_NAME_LEN (default 64) in the littlefs Kconfig.
const LITTLEFS_NAME_MAX: &str = "64";
/// Hardcoded by the littlefs component: CONFIG_LITTLEFS_BLOCK_SIZE = 4096.
const LITTLEFS_BLOCK_SIZE: &str = "4096";
/// littlefs-python version pinned by the littlefs component
/// (image-building-requirements.txt).
const LITTLEFS_PYTHON_VERSION: &str = "0.15.0";
/// littlefs partition offset, view with "espflash partition-table partitions.csv".
const LITTLEFS_PARTITION_OFFSET: u64 = 0x310000;
/// Default baud for the littlefs image transfer.
///
/// The board exposes the ESP32-S3's native USB-Serial-JTAG (e.g.
/// /dev/cu.usbmodem*), which is prone to "Timeout while running FlashDeflData
/// command" drops at the default 921600 baud once the write runs long enough.
/// The image is ~13.5 MB on flash but only ~180 KB compressed (mostly 0xFF
/// padding), so the failure is link reliability, not payload size. Override
/// with the ESPFLASH_BAUD environment variable (also honored by espflash).
const LITTLEFS_FLASH_BAUD: &str = "460800";
/// The 13.5 MB image is written in chunks with a fresh espflash connection per
/// chunk: a drop then only loses one small chunk instead of the whole image.
const LITTLEFS_FLASH_CHUNK_SIZE: usize = 2 * 1024 * 1024; // 2 MiB

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    let task = args.nth(1);
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();
    match task.as_deref() {
        Some("generate") => generate(&workspace_root)?,
        Some("flash") => flash(args, &workspace_root)?,
        Some("littlefsgen") => littlefsgen(args, &workspace_root)?,
        Some("monitor") => monitor(&workspace_root)?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

generate
    generate esp_board_manager code in components/gen_bmgr_codes
flash firmware|littlefs
    flash release firmware or target/littlefs.bin
    (littlefs is written in 2 MiB chunks at 460800 baud for USB-Serial-JTAG
    reliability; set ESPFLASH_BAUD to override the baud)
littlefsgen <video-directory>
    build LittleFS filesystem image with embedded videos
monitor
    monitor logs
"
    )
}

fn generate(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    println!("==> Initializing ESP-IDF environment and fetching components...");
    Command::new("cargo")
        .current_dir(workspace_root.as_ref())
        .args(["check", "--release"])
        .status()
        .context("`cargo check` failed")?;

    let python_path = find_python_path(workspace_root.as_ref())?;

    let bmgr_script_glob = workspace_root.as_ref().join("target/xtensa-esp32s3-espidf/release/build/esp-idf-sys-*/out/managed_components/espressif__esp_board_manager/gen_bmgr_config_codes.py");

    let mut bmgr_script_paths: Vec<_> = glob(
        bmgr_script_glob
            .to_str()
            .ok_or(anyhow::anyhow!("Failed to read BMGR glob pattern"))?,
    )
    .context("Failed to read BMGR glob pattern")?
    .filter_map(Result::ok)
    .collect();

    if bmgr_script_paths.is_empty() {
        panic!(
            "BMGR script not found in target directory. Ensure `remote_component` is in Cargo.toml."
        );
    }

    bmgr_script_paths.sort_by(|a, b| {
        let time_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let time_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    let bmgr_script = &bmgr_script_paths[0];
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
        Some("firmware") => {
            Command::new("espflash")
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
            Ok(())
        }
        Some("littlefs") => flash_littlefs(workspace_root),
        _ => Err(anyhow::anyhow!("specify `firmware` or `littlefs`")),
    }
}

fn flash_littlefs(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let image = workspace_root.as_ref().join("target/littlefs.bin");
    let data = fs::read(&image)
        .context("Failed to read target/littlefs.bin; run `cargo xtask littlefsgen` first")?;
    let chunks_dir = workspace_root.as_ref().join("target/littlefs-chunks");
    fs::create_dir_all(&chunks_dir).context("Failed to create chunk dir")?;

    let chunk_count = data.chunks(LITTLEFS_FLASH_CHUNK_SIZE).count();
    let baud = env::var("ESPFLASH_BAUD").unwrap_or_else(|_| LITTLEFS_FLASH_BAUD.to_string());
    println!(
        "==> Flashing {} bytes to 0x{:x} in {} chunks at {} baud...",
        data.len(),
        LITTLEFS_PARTITION_OFFSET,
        chunk_count,
        baud
    );

    let mut offset = LITTLEFS_PARTITION_OFFSET;
    for (i, chunk) in data.chunks(LITTLEFS_FLASH_CHUNK_SIZE).enumerate() {
        let chunk_path = chunks_dir.join(format!("chunk_{:02}.bin", i));
        fs::write(&chunk_path, chunk).context("Failed to write chunk file")?;

        println!(
            "==> Chunk {}/{}: offset 0x{:x} ({} bytes)...",
            i + 1,
            chunk_count,
            offset,
            chunk.len()
        );
        let status = Command::new("espflash")
            .current_dir(workspace_root.as_ref())
            .args([
                "write-bin",
                "--chip",
                "esp32s3",
                "--baud",
                &baud,
                &format!("0x{:x}", offset),
                chunk_path
                    .to_str()
                    .ok_or(anyhow::anyhow!("Invalid chunk path"))?,
            ])
            .status()
            .context("`espflash` littlefs failed")?;
        if !status.success() {
            return Err(anyhow::anyhow!(
                "Failed to flash chunk {}/{} at 0x{:x}. The link dropped mid-write; \
                 simply rerun `cargo xtask flash littlefs` - chunks already flashed are idempotent.",
                i + 1,
                chunk_count,
                offset
            ));
        }
        offset += chunk.len() as u64;
    }
    Ok(())
}

fn littlefsgen(mut args: env::Args, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let videos_path = args
        .next()
        .ok_or(anyhow::anyhow!("Missing path to a directory of videos"))?;
    let videos_path = Path::new(&videos_path);

    let littlefs_python = ensure_littlefs_python(workspace_root.as_ref())?;
    let status = Command::new(littlefs_python)
        .current_dir(workspace_root.as_ref())
        .args([
            "create",
            videos_path
                .to_str()
                .ok_or(anyhow::anyhow!("Invalid video path"))?,
            "target/littlefs.bin",
            "-v",
            "--fs-size=0xCF0000", // Must match littlefs partition size in partitions.csv
            "--name-max",
            LITTLEFS_NAME_MAX, // Must match CONFIG_LITTLEFS_OBJ_NAME_LEN
            "--block-size",
            LITTLEFS_BLOCK_SIZE,
        ])
        .status()
        .context("littlefsgen failed")?;

    if !status.success() {
        return Err(anyhow::anyhow!("littlefsgen failed"));
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
        let status = Command::new(&python_path)
            .args([
                "-m",
                "pip",
                "install",
                &format!("littlefs-python=={}", LITTLEFS_PYTHON_VERSION),
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
