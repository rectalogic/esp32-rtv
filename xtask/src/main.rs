use anyhow::Context;
use glob::glob;
use std::{env, fs, path::PathBuf, process::Command};

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    let task = args.nth(1);
    match task.as_deref() {
        Some("generate") => generate()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

generate       generate esp_board_manager code in components/gen_bmgr_codes
"
    )
}

fn generate() -> anyhow::Result<()> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();

    println!("==> Initializing ESP-IDF environment and fetching components...");
    Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["check", "--release"])
        .status()
        .context("`cargo check` failed")?;

    let python_glob = if cfg!(windows) {
        workspace_root.join(".embuild/espressif/python_env/idf*_py*_env/Scripts/python.exe")
    } else {
        workspace_root.join(".embuild/espressif/python_env/idf*_py*_env/bin/python")
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

    let bmgr_script_glob = workspace_root.join("target/xtensa-esp32s3-espidf/release/build/esp-idf-sys-*/out/managed_components/espressif__esp_board_manager/gen_bmgr_config_codes.py");

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
        .arg(workspace_root)
        .arg("-b")
        .arg("e32c28p") // custom board name
        .status()
        .context("Failed to execute BMGR generator")?;

    if !status.success() {
        return Err(anyhow::anyhow!("BMGR code generation failed"));
    }

    Ok(())
}
