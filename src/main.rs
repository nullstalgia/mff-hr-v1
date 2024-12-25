#![deny(unused_must_use)]

use app::App;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};

use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, Window};
use log::{error, info};
mod app;
mod errors;
mod heart_rate;
// mod littlefs;
mod settings;
mod touch;

use crate::errors::Result;

fn main() -> Result<()> {
    let mut display = SimulatorDisplay::<Rgb565>::new(Size::new(240, 320));

    display.clear(Rgb565::BLACK)?;
    let output_settings = OutputSettingsBuilder::new().build();
    let mut window = Window::new("Hello World", &output_settings);
    let mut app = App::build(display)?;
    loop {
        app.main_loop()?;
        window.update(&app.display);
    }

    // Ok(())
}
