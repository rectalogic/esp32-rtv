use anyhow::Context;
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
        Some("generate") => generate(&workspace_root)?,
        Some("flash") => flash(args, &workspace_root)?,
        Some("spiffsgen") => spiffsgen(args, &workspace_root)?,
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
flash firmware|spiffs
    flash release firmware or target/spiffs.bin
spiffsgen <video-directory>
    build SPIFF filesystem with embedded videos
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
        Some("spiffs") => {
            Command::new("espflash")
                .current_dir(workspace_root.as_ref())
                .args([
                    "write-bin",
                    // spiffs parition offset, view with "espflash partition-table partitions.csv"
                    "0x310000",
                    "target/spiffs.bin",
                ])
                .status()
                .context("`espflash` spiffs failed")?;
            Ok(())
        }
        _ => Err(anyhow::anyhow!("specify `firmware` or `spiffs")),
    }
}

fn spiffsgen(mut args: env::Args, workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
    let videos_path = args
        .next()
        .ok_or(anyhow::anyhow!("Missing path to a directory of videos"))?;
    let videos_path = Path::new(&videos_path);

    let python_path = find_python_path(workspace_root.as_ref())?;
    let status = Command::new(python_path)
        .current_dir(workspace_root.as_ref())
        .arg(find_esp_idf_path(workspace_root.as_ref())?.join("components/spiffs/spiffsgen.py"))
        .arg("0xCF0000") // Must match spiffs partition size in partitions.csv
        .arg(videos_path)
        .arg("target/spiffs.bin")
        .status()
        .context("spiffsgen failed")?;

    if !status.success() {
        return Err(anyhow::anyhow!("spiffsgen failed"));
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

fn find_esp_idf_path(workspace_root: impl AsRef<Path>) -> anyhow::Result<PathBuf> {
    Ok(workspace_root
        .as_ref()
        .join(".embuild/espressif/esp-idf")
        .join(env::var("ESP_IDF_VERSION")?))
}
