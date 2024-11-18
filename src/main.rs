use std::convert::Infallible;

use embedded_hal::digital::{ErrorType, PinState};
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{IOPin, OutputPin, PinDriver},
    prelude::Peripherals,
    spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2},
};

use esp_idf_hal::units::*;
use mipidsi::models::ST7789;

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    use display_interface_spi::SPIInterface;
    use embedded_graphics::{
        mono_font::{ascii::FONT_6X10, MonoTextStyle},
        pixelcolor::Rgb565,
        prelude::*,
        text::{Alignment, Text},
    };
    use embedded_hal::digital::{ErrorType, PinState};
    // use esp_idf_hal::delay::Delay;
    use esp_idf_hal::delay::Delay;
    // use ili9341::{DisplaySize240x320, Ili9341, Orientation};

    let peripherals = Peripherals::take()?;

    let lcd_spi = peripherals.spi2;
    let lcd_rs = peripherals.pins.gpio2;
    let mut lcd_rs = PinDriver::output(lcd_rs)?;
    // let lcd_dc = peripherals.pins.gpio2;
    // let mut lcd_dc = PinDriver::output(lcd_dc)?;
    let lcd_cs = peripherals.pins.gpio15;

    let mut delay: Delay = Default::default();
    // let dummy_reset = DummyOutputPin::default();
    let mut lcd_bl = PinDriver::output(peripherals.pins.gpio21)?;

    lcd_bl.set_high()?;

    let mut led = PinDriver::output(peripherals.pins.gpio16)?;
    let sclk = peripherals.pins.gpio14;
    let serial_in = peripherals.pins.gpio12; // SDI
    let serial_out = peripherals.pins.gpio13; // SDO
    let driver = SpiDriver::new::<SPI2>(
        lcd_spi,
        sclk,
        serial_out,
        Some(serial_in),
        &SpiDriverConfig::new(),
    )?;
    use mipidsi::Builder;
    let config_1 = esp_idf_hal::spi::config::Config::new().baudrate(13.MHz().into());
    let mut device_1 = SpiDeviceDriver::new(&driver, Some(lcd_cs), &config_1)?;
    let di = SPIInterface::new(device_1, lcd_rs);
    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        // .reset_pin(lcd_rs)
        .init(&mut delay)
        .unwrap();

    // Make the display all black
    display.clear(Rgb565::BLACK).unwrap();

    // let mut lcd = Ili9341::new(
    //     spi_iface,
    //     led,
    //     &mut delay,
    //     Orientation::PortraitFlipped,
    //     DisplaySize240x320,
    // )
    // .unwrap();
    // let mut lcd = ST7789::new(spi_iface, Some(lcd_rs), Some(lcd_bl), 320, 240);
    // Create a new character style
    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::RED);
    // Create a text at position (20, 30) and draw it using the previously defined style

    loop {
        Text::with_alignment(
            "First line\nSecond line",
            Point::new(20, 30),
            style,
            Alignment::Center,
        )
        .draw(&mut display)
        .unwrap();
        FreeRtos::delay_ms(1000);
    }

    Ok(())
}
