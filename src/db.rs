use std::fs::DirBuilder;

use directories_next::ProjectDirs;

pub(crate) fn create_dir() {
    if let Some(dir) = ProjectDirs::from("com", "sigaloid", "please-rs") {
        let cfg_dir = dir.config_dir();
        if !cfg_dir.exists() {
            DirBuilder::new().recursive(true).create(cfg_dir).ok();
        }
    }
}
