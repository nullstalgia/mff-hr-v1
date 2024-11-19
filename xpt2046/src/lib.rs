#![no_std]

mod calibration;
mod errors;
mod xpt2046;
use embedded_graphics::prelude::Point;
pub use xpt2046::Xpt2046;
// pub use errors::Error;
// pub(crate) use errors::Result;

// Guts stolen from:
// https://github.com/witnessmenow/ESP32-Cheap-Yellow-Display/tree/c4c60bf802afd817e28b223ecc32f0bdc7189f09/Variants/3248S035C/Examples/1-Draw/touchscreen
// Which is a descendant of:
// https://github.com/tommy-gilligan/touchscreen/tree/a5c286eb9bd47d53c4837c0681158c390fc9841d

// Cheers to all involved!

#[derive(Debug, PartialEq, Eq)]
pub enum TouchKind {
    Start,
    Move,
    End,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TouchEvent {
    pub point: Point,
    pub kind: TouchKind,
}

pub trait TouchScreen {
    type TouchError;

    fn get_touch_event(&mut self) -> ::core::result::Result<Option<TouchEvent>, Self::TouchError>;
}
