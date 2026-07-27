use anyhow::Context;
use glob::glob;
use std::{env, path::PathBuf, process::Command};

fn main() -> anyhow::Result<()> {
    let mut args = env::args();
    let task = args.nth(1);
    match task.as_deref() {
        Some("build") => build()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        "Tasks:

build       build firmware
"
    )
}

fn build() -> anyhow::Result<()> {
    // 1. Locate the workspace root and the firmware crate directory
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or(anyhow::anyhow!("Failed to find workspace root"))?
        .to_path_buf();

    // 2. Initialize the embuild environment and download components
    // We run `cargo check` on the firmware. It will likely fail at the CMake stage
    // because the generated C code is missing, but it WILL successfully download ESP-IDF,
    // create the Python venv, and fetch `esp_board_manager` into `managed_components/`.
    println!("==> Initializing ESP-IDF environment and fetching components...");
    Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["check"])
        .status()
        .context("`cargo check` failed")?;

    // 3. Locate the embuild-managed Python executable
    let python_glob = if cfg!(windows) {
        workspace_root.join(".embuild/espressif/python_env/idf*_py*_env/Scripts/python.exe")
    } else {
        workspace_root.join(".embuild/espressif/python_env/idf*_py*_env/bin/python")
    };

    let python_path = glob(python_glob.to_str().unwrap())
        .expect("Failed to read glob pattern")
        .filter_map(Result::ok)
        .next()
        .context("Could not find embuild-managed Python environment.")?;

    // 4. Locate the BMGR generator script
    let bmgr_script = workspace_root
        .join("managed_components/espressif__esp_board_manager/gen_bmgr_config_codes.py");
    if !bmgr_script.exists() {
        return Err(anyhow::anyhow!(
            "BMGR script not found at {}. Ensure `idf_component.yml` is configured correctly.",
            bmgr_script.display()
        ));
    }

    // 5. Generate the C code
    println!("==> Generating BMGR C code...");
    let status = Command::new(python_path)
        .current_dir(&workspace_root)
        .arg(bmgr_script)
        .arg("-b")
        .arg("e32c28p") // custom board name
        .status()
        .context("Failed to execute BMGR generator")?;

    if !status.success() {
        return Err(anyhow::anyhow!("BMGR code generation failed"));
    }

    // 6. Build the actual firmware
    println!("==> Building firmware...");
    let status = Command::new("cargo")
        .current_dir(&workspace_root)
        .args(["build"])
        .status()
        .context("Failed to run `cargo build`")?;

    if !status.success() {
        return Err(anyhow::anyhow!("Failed to run `cargo build`"));
    }
    Ok(())
}
