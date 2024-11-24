use crate::{calibration::CalibrationData, TouchEvent, TouchKind, TouchScreen};
use embedded_graphics::prelude::Point;
use embedded_hal::spi::SpiDevice;

mod spi;

pub struct Xpt2046<SPI>
where
    SPI: SpiDevice,
    // CALIB: Fn((u16, u16)) -> Option<(i32, i32)>,
{
    spi: spi::Spi<SPI>,
    last_touch: Option<Point>,
    pub(crate) calibration: Option<CalibrationData>,
}

impl<SPI> Xpt2046<SPI>
where
    SPI: SpiDevice,
    // CALIB: Fn((u16, u16)) -> Option<(i32, i32)>,
{
    pub fn new(touch_spi_device: SPI, calibration: Option<CalibrationData>) -> Self {
        Self {
            spi: spi::Spi::new(touch_spi_device),
            last_touch: None,
            calibration,
        }
    }
    pub fn calibrated(&self) -> bool {
        self.calibration.is_some()
    }
}

fn out_of_range((x, y): (u16, u16)) -> bool {
    if x < 250 || y < 230 || x > 4000 || y > 3900 {
        true
    } else {
        false
    }
}

impl<SPI> TouchScreen for Xpt2046<SPI>
where
    SPI: SpiDevice,
    // CALIB: Fn((u16, u16)) -> Option<(i32, i32)>,
{
    type TouchError = <SPI as embedded_hal::spi::ErrorType>::Error;

    fn get_touch_event(&mut self) -> Result<Option<TouchEvent>, Self::TouchError> {
        let raw_touch = self.spi.get()?;
        // let out_of_range = ;
        if out_of_range(raw_touch) {
            if let Some(last_touch) = self.last_touch {
                self.last_touch = None;

                Ok(Some(TouchEvent {
                    point: last_touch,
                    kind: TouchKind::End,
                }))
            } else {
                Ok(None)
            }
        } else {
            let (x, y) = match self.calibration.as_ref() {
                Some(affine_offset) => {
                    let x: u16 = (affine_offset.alpha_x * raw_touch.0 as f64
                        + affine_offset.beta_x * raw_touch.1 as f64
                        + affine_offset.delta_x) as u16;

                    let y: u16 = (affine_offset.alpha_y * raw_touch.0 as f64
                        + affine_offset.beta_y * raw_touch.1 as f64
                        + affine_offset.delta_y) as u16;

                    (x, y)
                }
                None => raw_touch,
            };
            let result = Some(TouchEvent {
                point: Point::new(x as i32, y as i32),
                kind: if self.last_touch.is_some() {
                    TouchKind::Move
                } else {
                    TouchKind::Start
                },
            });
            self.last_touch = Some(Point::new(x as i32, y as i32));

            Ok(result)
        }
    }
}
