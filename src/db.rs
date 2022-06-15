use std::fs::DirBuilder;

use directories_next::ProjectDirs;
use pickledb::PickleDb;
use time::OffsetDateTime;

pub(crate) fn create_dir() {
    if let Some(dir) = ProjectDirs::from("com", "sigaloid", "please-rs") {
        let cfg_dir = dir.config_dir();
        if !cfg_dir.exists() {
            DirBuilder::new().recursive(true).create(cfg_dir).ok();
        }
    }
}

pub(crate) fn get_weather(db: &mut PickleDb) -> Option<String> {
    let timestamp_current = get_time().unix_timestamp();
    let cache_weather = |db: &mut PickleDb| -> Option<String> {
        let city = db.get::<String>("weather-city").unwrap_or_default();
        let get = ureq::get(&format!("https://wttr.in/{}?format=\"%l:+%C+%c+%t\"", city))
            .call()
            .ok()?
            .into_string()
            .ok()?
            .replace("\"", "");
        db.set("weather-cached", &get)
            .expect("Failed to set cached weather");
        db.set("weather-timestamp", &timestamp_current)
            .expect("Failed to set cached weather");
        Some(get)
    };
    if let Some(timestamp) = db.get::<i64>("weather-timestamp") {
        if timestamp_current - timestamp > 3600 || !db.exists("weather-cached") {
            cache_weather(db)
        } else {
            db.get::<String>("weather-cached")
        }
    } else {
        cache_weather(db)
    }
}
pub(crate) fn get_time() -> OffsetDateTime {
    OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc())
}
