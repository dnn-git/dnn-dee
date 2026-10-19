//! Quét thư mục nhạc.

use std::path::{Path, PathBuf};

const EXTENSIONS: [&str; 8] = [
    "mp3", "flac", "wav", "ogg", "m4a", "aac", "opus", "wma",
];

/// Quét đệ quy, bỏ qua thư mục ẩn. Trong mỗi thư mục, tệp được sắp xếp
/// không phân biệt hoa thường, tệp ở thư mục cha đứng trước thư mục con.
pub fn scan(folder: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    walk(folder, &mut found);
    found
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    let mut files: Vec<(String, PathBuf)> = Vec::new();
    let mut dirs: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => {
                if !name.starts_with('.') {
                    dirs.push(path);
                }
            }
            Ok(_) => {
                if is_music(&path) {
                    files.push((name.to_lowercase(), path));
                }
            }
            Err(_) => {}
        }
    }

    files.sort_by(|a, b| a.0.cmp(&b.0));
    dirs.sort();

    out.extend(files.into_iter().map(|(_, path)| path));
    for sub in dirs {
        walk(&sub, out);
    }
}

fn is_music(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

/// Tên bài hát hiển thị = tên tệp bỏ phần mở rộng.
pub fn stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}
