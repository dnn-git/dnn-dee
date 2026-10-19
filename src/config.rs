//! Đọc/ghi cấu hình tại ~/.config/dee/config.json.
//! Định dạng giống bản Python nên cấu hình cũ dùng lại được.

use std::path::PathBuf;

use gtk::glib;
use serde_json::{Map, Value};

fn config_path() -> PathBuf {
    glib::user_config_dir().join("dee").join("config.json")
}

fn read_all() -> Map<String, Value> {
    std::fs::read_to_string(config_path())
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| match value {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .unwrap_or_default()
}

pub fn get_string(key: &str) -> Option<String> {
    read_all()
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

pub fn set_string(key: &str, value: &str) {
    let mut map = read_all();
    map.insert(key.to_owned(), Value::String(value.to_owned()));

    let path = config_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(&Value::Object(map)) {
        let _ = std::fs::write(path, text);
    }
}
