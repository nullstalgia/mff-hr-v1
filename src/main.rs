#![deny(unused_must_use)]

use app::App;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use esp_idf_hal::{
    delay::{Delay, FreeRtos},
    gpio::PinDriver,
    prelude::*,
    spi::{SpiDeviceDriver, SpiDriver, SpiDriverConfig, SPI2},
    // units::*,
};
use esp_idf_svc::fs::fatfs::Fatfs;
use esp_idf_svc::hal::gpio::AnyIOPin;
use esp_idf_svc::hal::sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver};
use esp_idf_svc::hal::spi::{config::DriverConfig, Dma};
use esp_idf_svc::io::vfs::MountedFatfs;
use mipidsi::interface::SpiInterface;

use esp_idf_sys::{self as _};
use littlefs::paths::*;
use log::{error, info};
use mipidsi::{
    models::ST7789,
    options::{Orientation, Rotation},
    Builder,
};
use xpt2046::{TouchEvent, TouchKind};

mod app;
mod errors;
mod heart_rate;
mod littlefs;
mod settings;
mod touch;

use std::{fs, sync::mpsc::TrySendError};

use crate::errors::Result;

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let _mounted_eventfs = esp_idf_svc::io::vfs::MountedEventfs::mount(5)?;

    info!(
        "My code is running! Core: {:?}, Heap free: {}",
        esp_idf_hal::cpu::core(),
        unsafe { esp_idf_hal::sys::esp_get_free_heap_size() }
    );
    let free_stack = unsafe { esp_idf_hal::sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut()) };
    info!("Stack Free: {free_stack}");
    let peripherals = Peripherals::take()?;
    let mut delay: Delay = Default::default();
    let io0 = {
        let pin = peripherals.pins.gpio0;
        let pin = PinDriver::input(pin)?;
        pin
    };
    // Waits a moment at startup to allow user to hold BOOT button
    // delay.delay_ms(1000);
    // If it's held down, littlefs will be formatted.
    littlefs::init_littlefs_storage(io0.is_low())?;

    let hspi = peripherals.spi2;
    let lcd_dc = {
        let pin = peripherals.pins.gpio2;
        let pin = PinDriver::output(pin)?;
        pin
    };
    let lcd_cs = peripherals.pins.gpio15;
    let lcd_clk = peripherals.pins.gpio14;
    let lcd_miso = peripherals.pins.gpio12; // TFT_SDO
    let lcd_mosi = peripherals.pins.gpio13; // TFT_SDI
    let driver = SpiDriver::new::<SPI2>(
        hspi,
        lcd_clk,
        lcd_mosi,
        Some(lcd_miso),
        &DriverConfig::default().dma(Dma::Auto(4096)),
    )?;
    let mut lcd_backlight = PinDriver::output(peripherals.pins.gpio21)?;
    lcd_backlight.set_low()?;

    let config_1 = esp_idf_hal::spi::config::Config::new().baudrate(80.MHz().into());
    let device_1 = SpiDeviceDriver::new(driver, Some(lcd_cs), &config_1)?;
    let mut buffer = [0_u8; 512];
    let di = SpiInterface::new(device_1, lcd_dc, &mut buffer);
    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ST7789, di)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .init(&mut delay)
        .unwrap();
    display.clear(Rgb565::BLACK)?;
    lcd_backlight.set_high()?;

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
    // let touch_irq = {
    //     let pin = peripherals.pins.gpio36;
    //     let mut pin = PinDriver::input(pin)?;
    //     pin.set_interrupt_type(InterruptType::PosEdge)?;
    //     pin.enable_interrupt()?;
    //     pin
    // };
    let touch_miso = {
        let pin = peripherals.pins.gpio39;
        let pin = PinDriver::input(pin)?;
        pin
    };
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

    use xpt2046::{CalibrationData, TouchScreen, Xpt2046};

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
        // Display is uncalibrated, resolve that before we do anything else.
        let output = touch.intrusive_calibration(&mut display, &mut delay)?;
        info!("{output:#?}");
        fs::write(
            TOUCH_CAL_PATH,
            postcard::to_vec::<CalibrationData, 512>(&output)?,
        )?;
    }

    let (touch_tx, touch_rx) = std::sync::mpsc::sync_channel::<Option<TouchEvent>>(0);

    std::thread::Builder::new()
        .stack_size(2000)
        .spawn(move || {
            let mut blocking_item = None;
            loop {
                match touch.get_touch_event() {
                    Ok(event) => {
                        let blocking_send = event
                            .as_ref()
                            .map(|e| e.kind != TouchKind::Move)
                            .unwrap_or(true);
                        if blocking_send && blocking_item.is_none() {
                            blocking_item = Some(event.clone());
                        }

                        let item_to_send = blocking_item.as_ref().unwrap_or(&event);

                        match touch_tx.try_send(item_to_send.to_owned()) {
                            Ok(()) => {
                                _ = blocking_item.take();
                            }
                            // If it's full, try again next loop run
                            Err(TrySendError::Full(_event)) => (),
                            Err(TrySendError::Disconnected(_event)) => panic!(),
                        }
                    }

                    Err(e) => {
                        panic!("{e}")
                    }
                }
                FreeRtos::delay_ms(5);
            }
        })?;

    assert_eq!(unsafe { esp_idf_hal::sys::esp_task_wdt_deinit() }, 0);

    let vspi = peripherals.spi3;
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
    let mounted_fatfs: Option<MountedFatfs<_>> =
        match MountedFatfs::mount(Fatfs::new_sdcard(0, sd_card_driver)?, "/sdcard", 4) {
            Ok(fs) => Some(fs),
            Err(e) => {
                error!("Failed mounting SD: {e}");
                None
            }
        };

    if mounted_fatfs.is_some() {
        for file in fs::read_dir("/sdcard")? {
            info!("{file:?}");
        }
    }

    // // let mut last_point = None;
    // std::thread::Builder::new()
    //     .stack_size(8000)
    //     .spawn(move || -> Result<(), anyhow::Error> {
    //         // let image =
    //         //     tinygif::Gif::<Rgb565>::from_slice(include_bytes!("../gifs/boykisser-2.gif"))
    //         //         .unwrap();
    //         info!(
    //             "Core: {:?}, Heap free: {}",
    //             esp_idf_hal::cpu::core(),
    //             unsafe { esp_idf_hal::sys::esp_get_free_heap_size() },
    //         );
    //         // let mut canvas: Canvas<Rgb565> = Canvas::new(Size::new(320, 100));
    //         // canvas.clear(Rgb565::BLACK).unwrap();
    //         let mut last_point = None;
    //         // let mut sub = display.cropped(&Rectangle::new(Point::zero(), Size::new(100, 100)));
    //         loop {
    //             // delay.delay_ms(10);
    //         }
    //     })
    //     .unwrap();

    let _ble_device = esp32_nimble::BLEDevice::take();

    let mut app = App::build(touch_rx, display, delay)?;
    let free_stack = unsafe { esp_idf_hal::sys::uxTaskGetStackHighWaterMark(std::ptr::null_mut()) };
    info!("Stack Free: {free_stack}");
    // app.change_view(crate::app::AppView::BadgeDisplay)?;
    loop {
        delay.delay_ms(10);
        app.main_loop()?;
    }

    // Ok(())
}
