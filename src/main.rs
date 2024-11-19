use display_interface_spi::SPIInterface;
use embedded_canvas::{Canvas, CanvasAt};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Polyline, PrimitiveStyle, Rectangle, StyledDrawable},
    text::{Alignment, Text},
};
use esp_idf_hal::{
    delay::{Delay, FreeRtos},
    gpio::{InterruptType, PinDriver, Pull},
    prelude::Peripherals,
    spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2, SPI3},
};
use log::{error, info};
use mipidsi::{
    models::ST7789,
    options::{Orientation, Rotation},
    Builder,
};

mod littlefs;
mod touch_calibrate;
// mod xpt2046;

use std::{
    fs,
    path::{Path, PathBuf},
};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!(
        "Core: {:?}, Heap free: {}",
        esp_idf_hal::cpu::core(),
        unsafe { esp_idf_hal::sys::esp_get_free_heap_size() }
    );

    littlefs::init_littlefs_storage()?;

    // use xpt2046::Xpt2046;

    let peripherals = Peripherals::take()?;

    let mut delay: Delay = Default::default();

    let hspi = peripherals.spi2;
    let lcd_dc = {
        let pin = peripherals.pins.gpio2;
        let pin = PinDriver::output(pin)?;
        pin
    };
    let lcd_cs = peripherals.pins.gpio15;
    let mut lcd_backlight = PinDriver::output(peripherals.pins.gpio21)?;
    lcd_backlight.set_high()?;

    let lcd_clk = peripherals.pins.gpio14;
    let lcd_miso = peripherals.pins.gpio12; // TFT_SDO
    let lcd_mosi = peripherals.pins.gpio13; // TFT_SDI
    let driver = SpiDriver::new::<SPI2>(
        hspi,
        lcd_clk,
        lcd_mosi,
        Some(lcd_miso),
        &SpiDriverConfig::new(),
    )?;

    let config_1 = esp_idf_hal::spi::config::Config::new().baudrate(80.MHz().into());
    let device_1 = SpiDeviceDriver::new(&driver, Some(lcd_cs), &config_1)?;
    let di = SPIInterface::new(device_1, lcd_dc);
    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut delay)
        .unwrap();
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::BLACK)
        .unwrap();

    let vspi = peripherals.spi3;
    let touch_clk = peripherals.pins.gpio25;
    let touch_mosi = peripherals.pins.gpio32;
    let touch_cs = peripherals.pins.gpio33;
    let touch_irq = {
        let pin = peripherals.pins.gpio36;
        let mut pin = PinDriver::input(pin)?;
        pin.set_interrupt_type(InterruptType::PosEdge)?;
        pin.enable_interrupt()?;
        pin
    };
    let touch_miso = peripherals.pins.gpio39;
    use embedded_graphics::prelude::*;
    let vspi_driver = SpiDriver::new::<SPI3>(
        vspi,
        touch_clk,
        touch_mosi,
        Some(touch_miso),
        &SpiDriverConfig::new(),
    )?;

    use xpt2046::{CalibrationData, TouchScreen, Xpt2046};

    let touch_config = esp_idf_hal::spi::config::Config::new().baudrate(2.MHz().into());
    let spi_touch = SpiDeviceDriver::new(&vspi_driver, Some(touch_cs), &touch_config)?;

    let touch_path = PathBuf::from("/littlefs/touch_cal");
    let touch_calibration: Option<CalibrationData> = {
        if !touch_path.exists() {
            None
        } else {
            let data = fs::read(&touch_path)?;
            if let Ok(data) = postcard::from_bytes::<CalibrationData>(&data) {
                Some(data)
            } else {
                error!("Failed to deserialize touch calibration!");
                None
            }
        }
    };

    let mut touch = Xpt2046::new(spi_touch, touch_calibration);

    use esp_idf_hal::units::*;

    assert_eq!(unsafe { esp_idf_hal::sys::esp_task_wdt_deinit() }, 0);

    if !touch.calibrated() {
        let output = touch.intrusive_calibration(&mut display, &mut delay)?;
        info!("{output:#?}");
        fs::write(
            touch_path,
            postcard::to_vec::<CalibrationData, 512>(&output)?,
        )?;
    }

    let mut position = Point::new(00, 50);
    let style = MonoTextStyle::new(&FONT_6X10, Rgb565::RED);
    let mut last_text = Text::with_alignment("A", position, style, Alignment::Left);

    last_text.draw(&mut display).unwrap();

    // loop {}

    // let mut touch = Xpt2046::new(device_1, touch_irq, xpt2046::Orientation::Portrait);

    // // touch.set_inverted(Inverted::invert_all());

    // touch.init(&mut delay).unwrap();

    // touch.set_num_samples(16);

    // touch.calibrate(&mut display, &mut delay).unwrap();
    let mut canvas = CanvasAt::<Rgb565>::new(Point::new(40, 40), Size::new(300, 20));
    let mut old_bb: Rectangle = Default::default();
    let mut last_point = None;
    loop {
        // touch.run().unwrap();
        // let touchies = touch.is_touched();
        // // let meow = if touchies { "touchy" } else { "nooo" };
        // // info!("{meow}");
        // if touchies {
        //     let p = touch.get_touch_point();
        //     info!("x:{} y:{}", p.x, p.y);
        // }
        match touch.get_touch_event() {
            Ok(event) => {
                let a = format!("{event:?}");
                // info!("{a}");
                let text = Text::with_alignment(&a, position, style, Alignment::Left);
                _ = canvas.fill_solid(&old_bb, Rgb565::BLACK);
                _ = text.draw(&mut canvas);
                old_bb = text.bounding_box();
                if let Some(pixel) = event.as_ref() {
                    // display
                    //     .fill_solid(
                    //         &Rectangle::new(
                    //             Point::new(pixel.point.x, pixel.point.y),
                    //             Size::new_equal(2),
                    //         ),
                    //         Rgb565::BLUE,
                    //     )
                    //     .unwrap();
                    let point1 = if let Some(last) = last_point {
                        last
                    } else {
                        pixel.point
                    };
                    _ = Polyline::new(&[point1, pixel.point])
                        .draw_styled(&PrimitiveStyle::with_stroke(Rgb565::BLUE, 2), &mut display);
                    last_point = Some(pixel.point);
                } else {
                    last_point = None;
                }
                canvas
                    // .place_at(Point::new(40, 40))
                    .draw(&mut display)
                    .unwrap();
            }
            Err(e) => {
                error!("{e}");
            }
        }
        // FreeRtos::delay_ms(16);
    }

    // Ok(())
}
