use crate::{calibration::CalibrationData, TouchEvent, TouchKind, TouchScreen};
use embedded_graphics::prelude::Point;
use embedded_hal::spi::SpiDevice;

mod spi;

const SAMPLE_CAPACITY: usize = 10;
const SAMPLE_THRESHOLD: usize = 5;

pub struct Xpt2046<SPI>
where
    SPI: SpiDevice,
    // CALIB: Fn((u16, u16)) -> Option<(i32, i32)>,
{
    spi: spi::Spi<SPI>,
    touch_samples: heapless::Vec<(u16, u16), SAMPLE_CAPACITY>,
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
            calibration,
            touch_samples: heapless::Vec::new(),
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

        let samples_at_capacity = self.touch_samples.len() == self.touch_samples.capacity();

        if out_of_range(raw_touch) {
            if self.touch_samples.is_empty() {
                return Ok(None);
            } else {
                let last_touch = self.touch_samples[0];
                let last_touch = Point::new(last_touch.0 as i32, last_touch.1 as i32);

                self.touch_samples.clear();

                return if samples_at_capacity {
                    Ok(Some(TouchEvent {
                        point: last_touch,
                        kind: TouchKind::End,
                    }))
                } else {
                    Ok(None)
                };
            }
        }

        // let raw_point = Point::new(raw_touch.0 as i32, raw_touch.1 as i32);
        if samples_at_capacity {
            _ = self.touch_samples.pop();
        }
        // Unwrap should be okay due to making room just above
        self.touch_samples.insert(0, raw_touch).unwrap();
        if self.touch_samples.len() < SAMPLE_THRESHOLD {
            return Ok(None);
        }

        let (x, y) = match self.calibration.as_ref() {
            Some(affine_offset) => {
                // assert!(samples_at_capacity);

                // Samples should be full by now.
                let sum = self
                    .touch_samples
                    .iter()
                    .fold((0u32, 0u32), |(acc_x, acc_y), &(x, y)| {
                        (acc_x + x as u32, acc_y + y as u32)
                    });
                let len = self.touch_samples.len() as u32;
                let averaged = (sum.0 / len, sum.1 / len);

                let x: u16 = (affine_offset.alpha_x * averaged.0 as f64
                    + affine_offset.beta_x * averaged.1 as f64
                    + affine_offset.delta_x) as u16;

                let y: u16 = (affine_offset.alpha_y * averaged.0 as f64
                    + affine_offset.beta_y * averaged.1 as f64
                    + affine_offset.delta_y) as u16;

                (x, y)
                // raw_touch
            }
            None => raw_touch,
        };
        let result = Some(TouchEvent {
            point: Point::new(x as i32, y as i32),
            kind: if self.touch_samples.len() == SAMPLE_THRESHOLD {
                TouchKind::Start
            } else {
                TouchKind::Move
            },
        });

        Ok(result)
    }
}
