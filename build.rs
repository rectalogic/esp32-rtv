use glob::glob;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() {
    embuild::espidf::sysenv::output();

    symlink_compile_commands();
}

fn symlink_compile_commands() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    println!("cargo:warning={out_dir}"); //XXX
    let out_dir = PathBuf::from(&out_dir);

    let build_dir = out_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("build_dir");

    let path_glob = build_dir.join("esp-idf-sys-*");

    let mut paths: Vec<_> = glob(path_glob.to_str().expect("glob str"))
        .expect("glob")
        .filter_map(Result::ok)
        .collect();

    if paths.is_empty() {
        panic!("Path {path_glob:?} not found");
    }

    paths.sort_by(|a, b| {
        let time_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let time_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        time_b.cmp(&time_a)
    });

    let active_esp_dir = paths.swap_remove(0);

    // 2. Symlink the compile_commands.json from the active directory
    let cc_json_path = active_esp_dir.join("out/build/compile_commands.json");
    if cc_json_path.exists() {
        let target_link = Path::new("compile_commands.json");

        // Clear old links safely
        if target_link.exists() || target_link.is_symlink() {
            let _ = fs::remove_file(target_link);
        }

        #[cfg(unix)]
        let _ = std::os::unix::fs::symlink(&cc_json_path, target_link);

        #[cfg(windows)]
        let _ = std::os::windows::fs::symlink_file(&cc_json_path, target_link);
    }
}
