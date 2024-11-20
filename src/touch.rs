// use std::fs;

// use embedded_hal::spi::SpiDevice;
// use esp_idf_hal::gpio::OutputPin;
// use esp_idf_hal::spi::{SpiDeviceDriver, SpiDriver};
// use esp_idf_hal::units::*;
// use xpt2046::Xpt2046;

// use crate::littlefs::paths::*;

// pub struct TouchHandle {}

// struct TouchActor<'d, SpiSingleDeviceDriver> {
//     spi_driver: SpiDriver<'d>,
//     xpt: Xpt2046<SPI>,
// }

// impl<'d, SPI: SpiDevice, CS: OutputPin> TouchActor<'d, SPI, CS> {
//     fn build(touch_cs: CS) -> anyhow::Result<Self> {
//         let mut touch = Xpt2046::new(spi_touch, touch_calibration);

//         Ok(Self { xpt: touch })
//     }
// }
