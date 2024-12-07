use std::{fs, io::Write};

use crate::{
    app::SlideshowLength,
    errors::{AppError, Result},
    heart_rate::ble::BleIdents,
};
use derivative::Derivative;
use embassy_time::Duration;
use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HrSettings {
    pub saved: Option<BleIdents>,
}

#[derive(Debug, Deserialize, Serialize, Derivative)]
#[derivative(Default)]
pub struct Settings {
    #[derivative(Default(value = "String::from(\"Goobinski\")"))]
    pub username: String,
    pub hr: HrSettings,
    #[serde(default)]
    pub slideshow_length_sec: SlideshowLength,
}

const SETTINGS_PATH: &str = "/littlefs/settings";

impl Settings {
    pub fn littlefs_load() -> Result<Self> {
        if !fs::exists(SETTINGS_PATH)? {
            let default = Self::default();
            default.littlefs_save()?;
            Ok(default)
        } else {
            let bytes = fs::read(SETTINGS_PATH)?;
            let res = postcard::from_bytes::<Settings>(&bytes);
            let data = res.unwrap_or_default();
            Ok(data)
        }
    }
    pub fn littlefs_save(&self) -> Result<()> {
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(SETTINGS_PATH)?;
        postcard::to_io(self, file)?;
        Ok(())
    }
}
