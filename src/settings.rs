use std::{fs, io::Write};

use crate::{
    app::SlideshowLength,
    errors::{AppError, Result},
};
use derivative::Derivative;

use serde_derive::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Derivative)]
#[derivative(Default)]
pub struct Settings {
    #[derivative(Default(value = "String::from(\"nullstalgia\")"))]
    pub username: String,
    #[serde(default)]
    pub slideshow_length_sec: SlideshowLength,
}
