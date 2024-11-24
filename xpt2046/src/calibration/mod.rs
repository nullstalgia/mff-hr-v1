use crate::{xpt2046::Xpt2046, TouchKind};

// use embedded_canvas::CCanvas;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Line, Primitive, PrimitiveStyle},
    text::{Alignment, Text},
    Drawable,
};
// use embedded_graphics_core::{
//     draw_target::DrawTarget,
//     geometry::Point,
//     pixelcolor::{Rgb565, RgbColor},
// };

// #[cfg(feature = "with_defmt")]
// use defmt::{write, Format, Formatter};
use embedded_hal::{delay::DelayNs, spi::SpiDevice};
use serde_derive::{Deserialize, Serialize};

use crate::TouchScreen;

// A lot of logic yoinked from:
// https://github.com/ardnew/XPT2046_Calibrated/blob/8d3f8b518b617b6fbc870ef3229b27aa83028c56/src/XPT2046_Calibrated.cpp
// And inspired by:
// https://github.com/Yandrik/xpt2046/blob/8d8cf9481268f61580e3dccf90717bbbeb50aa99/src/calibration.rs
// and ancestors
#[derive(Copy, Clone, Debug, Default)]
// #[cfg_attr(feature = "defmt", derive(::defmt::Format))]
pub struct CalibrationPoint {
    /// The x coordinate.
    pub x: f64,

    /// The y coordinate.
    pub y: f64,
}

impl From<&Point> for CalibrationPoint {
    fn from(value: &Point) -> Self {
        Self {
            x: value.x as f64,
            y: value.y as f64,
        }
    }
}

impl From<Point> for CalibrationPoint {
    fn from(value: Point) -> Self {
        Self::from(&value)
    }
}

#[derive(Debug, Clone)]
pub struct CalibrationSet {
    pub a: CalibrationPoint,
    pub b: CalibrationPoint,
    pub c: CalibrationPoint,
}

impl Default for CalibrationSet {
    fn default() -> Self {
        Self {
            a: CALIBRATION_POINTS[0].into(),
            b: CALIBRATION_POINTS[1].into(),
            c: CALIBRATION_POINTS[2].into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibrationData {
    pub alpha_x: f64,
    pub beta_x: f64,
    pub delta_x: f64,
    pub alpha_y: f64,
    pub beta_y: f64,
    pub delta_y: f64,
}

impl Default for CalibrationData {
    fn default() -> Self {
        CalibrationData {
            alpha_x: 1.0,
            beta_x: 0.0,
            delta_x: 0.0,
            alpha_y: 0.0,
            beta_y: 1.0,
            delta_y: 0.0,
        }
    }
}

const CALIBRATION_POINTS: &[Point; 3] = &[
    Point::new(30, 30),
    Point::new(312, 113),
    Point::new(167, 214),
];

pub fn calibration_math(touch_space_points: &CalibrationSet) -> CalibrationData {
    let screen_space_points: CalibrationSet = Default::default();

    // Just shortening names for easier reading
    let screen = screen_space_points;
    let touch = touch_space_points;

    let delta = ((touch.a.x - touch.c.x) * (touch.b.y - touch.c.y))
        - ((touch.b.x - touch.c.x) * (touch.a.y - touch.c.y));

    let alpha_x = ((screen.a.x - screen.c.x) * (touch.b.y - touch.c.y)
        - (screen.b.x - screen.c.x) * (touch.a.y - touch.c.y))
        / delta;

    let beta_x = ((touch.a.x - touch.c.x) * (screen.b.x - screen.c.x)
        - (touch.b.x - touch.c.x) * (screen.a.x - screen.c.x))
        / delta;

    let delta_x = ((screen.a.x * ((touch.b.x * touch.c.y) - (touch.c.x * touch.b.y)))
        - (screen.b.x * ((touch.a.x * touch.c.y) - (touch.c.x * touch.a.y)))
        + (screen.c.x * ((touch.a.x * touch.b.y) - (touch.b.x * touch.a.y))))
        / delta;

    let alpha_y = ((screen.a.y - screen.c.y) * (touch.b.y - touch.c.y)
        - (screen.b.y - screen.c.y) * (touch.a.y - touch.c.y))
        / delta;

    let beta_y = ((touch.a.x - touch.c.x) * (screen.b.y - screen.c.y)
        - (touch.b.x - touch.c.x) * (screen.a.y - screen.c.y))
        / delta;

    let delta_y = ((screen.a.y * ((touch.b.x * touch.c.y) - (touch.c.x * touch.b.y)))
        - (screen.b.y * ((touch.a.x * touch.c.y) - (touch.c.x * touch.a.y)))
        + (screen.c.y * ((touch.a.x * touch.b.y) - (touch.b.x * touch.a.y))))
        / delta;

    CalibrationData {
        alpha_x,
        beta_x,
        delta_x,
        alpha_y,
        beta_y,
        delta_y,
    }
}

impl<SPI> Xpt2046<SPI>
where
    SPI: SpiDevice,
{
    /// Takes over the screen to calibrate touch input.
    pub fn intrusive_calibration<DRAW, DELAY>(
        &mut self,
        dt: &mut DRAW,
        delay: &mut DELAY,
    ) -> Result<CalibrationData, SPI::Error>
    where
        DRAW: DrawTarget<Color = Rgb565>,
        DELAY: DelayNs,
    {
        let mut points_tapped = 0;
        let mut given_input = CalibrationSet::default();

        // Prepare the screen for points
        let _ = dt.clear(Rgb565::BLACK);

        // Create a new character style
        let style = MonoTextStyle::new(&FONT_6X10, Rgb565::WHITE);

        // This should maybe be part of a builder for this whole struct, passing in
        // a font through the generics system.
        _ = Text::with_alignment(
            "Touchscreen Calibration\nTap the dots carefully, 3 total.",
            Point::new(320 / 2, 240 / 2),
            style,
            Alignment::Center,
        )
        .draw(dt);

        let mut is_pressed: bool = false;

        while points_tapped < 4 {
            let latest_event = self.get_touch_event()?;
            let latest_event_kind: Option<&TouchKind> = latest_event.as_ref().map(|e| &e.kind);
            match points_tapped {
                0 => {
                    calibration_draw_point(dt, &CALIBRATION_POINTS[0], is_pressed);
                    if let Some(TouchKind::Move) = latest_event_kind {
                        given_input.a = latest_event.unwrap().point.into();
                        is_pressed = true;
                    } else if let Some(TouchKind::End) = latest_event_kind {
                        delay.delay_ms(200);
                        points_tapped += 1;
                        is_pressed = false;
                    }
                }

                1 => {
                    calibration_draw_point(dt, &CALIBRATION_POINTS[1], is_pressed);
                    if let Some(TouchKind::Move) = latest_event_kind {
                        given_input.b = latest_event.unwrap().point.into();
                        is_pressed = true;
                    } else if let Some(TouchKind::End) = latest_event_kind {
                        delay.delay_ms(200);
                        points_tapped += 1;
                        is_pressed = false;
                    }
                }
                2 => {
                    calibration_draw_point(dt, &CALIBRATION_POINTS[2], is_pressed);
                    if let Some(TouchKind::Move) = latest_event_kind {
                        given_input.c = latest_event.unwrap().point.into();
                        is_pressed = true;
                    } else if let Some(TouchKind::End) = latest_event_kind {
                        delay.delay_ms(200);
                        points_tapped += 1;
                        is_pressed = false;
                    }
                }

                3 => {
                    _ = dt.clear(Rgb565::BLACK);
                    self.calibration = Some(calibration_math(&given_input));
                    return Ok(self.calibration.clone().unwrap());
                }
                _ => (),
            }
            delay.delay_ms(10);
        }

        unreachable!()
    }
}

/// This flickers if drawn without a framebuffer/canvas.
fn calibration_draw_point<DT: DrawTarget<Color = Rgb565>>(dt: &mut DT, p: &Point, pressed: bool) {
    let color = if pressed { Rgb565::RED } else { Rgb565::BLUE };

    _ = Circle::with_center(*p, 7)
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(dt);

    _ = Line::new(Point::new(p.x - 4, p.y), Point::new(p.x + 4, p.y))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(dt);
    _ = Line::new(Point::new(p.x, p.y - 4), Point::new(p.x, p.y + 4))
        .into_styled(PrimitiveStyle::with_stroke(Rgb565::WHITE, 1))
        .draw(dt);
}
