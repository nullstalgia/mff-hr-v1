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
    prelude::*,
    spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI1, SPI2, SPI3},
    units::*,
};
use esp_idf_sys::BIT16;
use esp_idf_sys::{self as _};
use littlefs::paths::*;
use log::{error, info};
use mipidsi::{
    models::ST7789,
    options::{Orientation, Rotation},
    Builder,
};
use xpt2046::TouchEvent;

// mod app;
mod littlefs;
mod touch;
// mod xpt2046;

use std::{
    fs,
    sync::mpsc::{TryRecvError, TrySendError},
};

fn main() -> anyhow::Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    info!(
        "My code is running! Core: {:?}, Heap free: {}",
        esp_idf_hal::cpu::core(),
        unsafe { esp_idf_hal::sys::esp_get_free_heap_size() }
    );

    littlefs::init_littlefs_storage()?;

    // std::thread::Builder

    let peripherals = Peripherals::take()?;
    let mut delay: Delay = Default::default();

    // let mut app = App::build(peripherals)?;

    let hspi = peripherals.spi2;
    let lcd_dc = {
        let pin = peripherals.pins.gpio2;
        let pin = PinDriver::output(pin)?;
        pin
    };
    let lcd_cs = peripherals.pins.gpio15;
    let mut lcd_backlight = PinDriver::output(peripherals.pins.gpio21)?;
    lcd_backlight.set_low()?;
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
    let device_1 = SpiDeviceDriver::new(driver, Some(lcd_cs), &config_1)?;
    let di = SPIInterface::new(device_1, lcd_dc);
    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut delay)
        .unwrap();
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::BLACK)
        .unwrap();
    lcd_backlight.set_high()?;
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::WHITE)
        .unwrap();
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::BLACK)
        .unwrap();
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::WHITE)
        .unwrap();
    display
        .clear(embedded_graphics::pixelcolor::Rgb565::BLACK)
        .unwrap();
    let vspi = peripherals.spi3;
    let touch_clk = {
        let pin = peripherals.pins.gpio25;
        let pin = PinDriver::output(pin)?;
        pin
    };
    let touch_mosi = {
        let pin = peripherals.pins.gpio32;
        let pin = PinDriver::output(pin)?;
        pin
    };
    let touch_cs = {
        let pin = peripherals.pins.gpio33;
        let pin = PinDriver::output(pin)?;
        pin
    };
    // // let touch_irq = {
    // //     let pin = peripherals.pins.gpio36;
    // //     let mut pin = PinDriver::input(pin)?;
    // //     pin.set_interrupt_type(InterruptType::PosEdge)?;
    // //     pin.enable_interrupt()?;
    // //     pin
    // // };
    let touch_miso = {
        let pin = peripherals.pins.gpio39;
        let pin = PinDriver::input(pin)?;
        pin
    };
    use embedded_graphics::prelude::*;

    let bitbang_spi = bitbang_hal::spi::Spi::build(
        bitbang_hal::spi::MODE_0,
        touch_miso,
        touch_mosi,
        touch_clk,
        touch_cs,
        delay.clone(),
    )?;
    // Just testing setting delay, works w/o
    // let bitbang_spi = bitbang_spi.with_delay_ns(100000);
    // bitbang_spi.set_delay_ns(100000);

    // let vspi_driver = SpiDriver::new::<SPI>(
    //     vspi,
    //     touch_clk,
    //     touch_mosi,
    //     Some(touch_miso),
    //     &SpiDriverConfig::new(),
    // )?;

    use xpt2046::{CalibrationData, TouchScreen, Xpt2046};

    // let touch_config = esp_idf_hal::spi::config::Config::new().baudrate(2.MHz().into());
    // let spi_touch = SpiDeviceDriver::new(vspi_driver, Some(touch_cs), &touch_config)?;

    let touch_calibration: Option<CalibrationData> = {
        if !fs::exists(TOUCH_CAL_PATH)? {
            None
        } else {
            let data = fs::read(TOUCH_CAL_PATH)?;
            if let Ok(data) = postcard::from_bytes::<CalibrationData>(&data) {
                Some(data)
            } else {
                error!("Failed to deserialize touch calibration!");
                None
            }
        }
    };

    let mut touch = Xpt2046::new(bitbang_spi, touch_calibration);

    if !touch.calibrated() {
        // display.draw_iter(pixels)
        let output = touch.intrusive_calibration(&mut display, &mut delay)?;
        info!("{output:#?}");
        fs::write(
            TOUCH_CAL_PATH,
            postcard::to_vec::<CalibrationData, 512>(&output)?,
        )?;
    }

    let (touch_tx, touch_rx) = std::sync::mpsc::sync_channel::<Option<TouchEvent>>(3);

    // std::thread::Builder::new()
    //     .stack_size(5000)
    //     .spawn(move || {
    //         loop {
    //             match touch.get_touch_event() {
    //                 Ok(event) => {
    //                     // info!("{event:?}");
    //                     match touch_tx.try_send(event) {
    //                         Ok(()) => (),
    //                         // If it's full, lets just block until we *can* send more.
    //                         Err(TrySendError::Full(event)) => touch_tx.send(event).unwrap(),
    //                         Err(TrySendError::Disconnected(_)) => panic!(),
    //                     }
    //                 }
    //                 Err(e) => {
    //                     panic!("{e}")
    //                 }
    //             }
    //             FreeRtos::delay_ms(100);
    //         }
    //     })?;

    assert_eq!(unsafe { esp_idf_hal::sys::esp_task_wdt_deinit() }, 0);

    use esp_idf_svc::fs::fatfs::Fatfs;
    use esp_idf_svc::hal::gpio::AnyIOPin;
    use esp_idf_svc::hal::prelude::*;
    use esp_idf_svc::hal::sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver};
    use esp_idf_svc::hal::spi::{config::DriverConfig, Dma, SpiDriver};
    use esp_idf_svc::io::vfs::MountedFatfs;
    use esp_idf_svc::log::EspLogger;

    let sd_cs = peripherals.pins.gpio5;
    let sd_sck = peripherals.pins.gpio18;
    let sd_miso = peripherals.pins.gpio19;
    let sd_mosi = peripherals.pins.gpio23;

    let spi_driver = SpiDriver::new(
        vspi,
        sd_sck,
        sd_mosi,
        Some(sd_miso),
        &DriverConfig::default().dma(Dma::Auto(4096)),
    )?;

    let sd_card_driver = SdCardDriver::new_spi(
        SdSpiHostDriver::new(
            spi_driver,
            Some(sd_cs),
            AnyIOPin::none(),
            AnyIOPin::none(),
            AnyIOPin::none(),
            None,
        )?,
        &SdCardConfiguration::new(),
    )?;

    // Keep it around or else it will be dropped and unmounted
    let _mounted_fatfs = MountedFatfs::mount(Fatfs::new_sdcard(0, sd_card_driver)?, "/sdcard", 4)?;
    info!("BB");

    for file in fs::read_dir("/sdcard")? {
        info!("{file:?}");
    }

    let text = fs::read_to_string("/sdcard/meow.txt")?;
    info!("{text}");
    let input = fs::File::open("/sdcard/BOYKIS~1.GIF").unwrap();
    // Configure the decoder such that it will expand the image to RGBA.
    for step in gift::Decoder::new(input) {
        let step = step?;
        // was there a decoding error?
        let raster = step.raster();
        use pix::el::Pixel;
        _ = display.fill_contiguous(
            &Rectangle::new(Point::zero(), Size::new(100, 75)),
            raster
                .pixels()
                .iter()
                .map(|p| Rgb565::new(p.one().into(), p.two().into(), p.three().into())),
        );
        // ... work with raster
    }

    // let mut last_point = None;
    // std::thread::Builder::new()
    //     .stack_size(80000)
    //     .spawn(move || -> Result<(), anyhow::Error> {
    // let image = tinygif::Gif::<{ 120 * 120 }, Rgb565>::from_slice(include_bytes!(
    // "../boykisser-3.gif"
    // ))
    // .unwrap();
    // unsafe { esp_idf_sys:: };
    info!(
        "Core: {:?}, Heap free: {}",
        esp_idf_hal::cpu::core(),
        unsafe { esp_idf_hal::sys::esp_get_free_heap_size() },
    );
    // let mut canvas: Canvas<Rgb565> = Canvas::new(Size::new(100, 100));
    // let mut sub = display.cropped(&Rectangle::new(Point::zero(), Size::new(100, 100)));
    // loop {
    // for frame in image.frames() {
    // info!("frame {:?}", frame);
    // frame.
    // _ = frame.draw(&mut sub);
    // _ = canvas.place_at(Point::zero()).draw(&mut display);
    // frame.draw(target)
    // _ = display.fill_contiguous(
    //     &canvas.bounding_box(),
    //     canvas.pixels.iter().filter_map(|&pixel| pixel),
    // );

    // delay.delay_ms(10);

    // let delay_ms = frame.delay_centis * 10;
    // info!("delay {}", delay_ms);
    // Delay here
    // Timer::after(Duration::from_millis(delay_ms as u64)).await;

    // Or, draw at given offset
    // use embedded_graphics::prelude::DrawTargetExt;
    // frame.draw(&mut display.translated(Point::new(30, 50))).unwrap();
    // }
    // }
    // })
    // .unwrap();

    loop {
        delay.delay_ms(10);
        // match touch_rx.try_recv() {
        //     Ok(Some(event)) => {
        //         let point1 = if let Some(last) = last_point {
        //             last
        //         } else {
        //             event.point
        //         };
        //         info!("Drawing!");
        //         _ = Polyline::new(&[point1, event.point])
        //             .draw_styled(&PrimitiveStyle::with_stroke(Rgb565::BLUE, 2), &mut display);
        //         last_point = Some(event.point);
        //     }
        //     Ok(None) => last_point = None,
        //     Err(TryRecvError::Empty) => (),
        //     Err(TryRecvError::Disconnected) => panic!("Touch DCd!"),
        // }
    }

    // Ok(())
}
