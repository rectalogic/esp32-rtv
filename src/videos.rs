use std::{ffi::OsStr, fs, path::Path};

pub fn find_videos(dir: impl AsRef<Path>) -> anyhow::Result<Vec<String>> {
    Ok(fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(OsStr::to_str)
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("mp4"))
                && !path
                    .file_name()
                    .is_some_and(|name| name.as_encoded_bytes().starts_with(b"._"))
        })
        .filter_map(|p| Some(p.to_str()?.to_owned()))
        .collect())
}
