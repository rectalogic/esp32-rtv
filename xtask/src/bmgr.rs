use anyhow::Context;
use glob::glob;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub fn bmgr(workspace_root: impl AsRef<Path>) -> anyhow::Result<()> {
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
