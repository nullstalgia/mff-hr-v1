//! Serial Peripheral Interface
//!
//! This implementation consumes the following hardware resources:
//! - Periodic timer to mark clock cycles
//! - Output GPIO pin for clock signal (SCLK)
//! - Output GPIO pin for data transmission (Master Output Slave Input - MOSI)
//! - Input GPIO pin for data reception (Master Input Slave Output - MISO)
//!
//! The timer must be configured to twice the desired communication frequency.
//!
//! SS/CS (slave select) must be handled independently.
//!
//! MSB-first and LSB-first bit orders are supported.
//!

use core::fmt::{self, Debug, Display};

pub use embedded_hal::spi::{MODE_0, MODE_1, MODE_2, MODE_3};

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::{self, ErrorType, Mode, Polarity, SpiDevice};

/// Error type
#[derive(Debug)]
pub enum Error<E: Debug + Display> {
    /// Communication error
    Bus(E),
    /// Attempted read without input data
    NoData,
}

impl<E: Debug + Display> core::error::Error for Error<E> {}

impl<E: Debug + Display> Display for Error<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Bus(e) => write!(f, "Bus error: {}", e),
            Error::NoData => write!(f, "NoData error"),
        }
    }
}

impl<E: Debug + Display> spi::Error for Error<E> {
    fn kind(&self) -> spi::ErrorKind {
        spi::ErrorKind::Other
    }
}

/// Transmission bit order
#[derive(Debug)]
pub enum BitOrder {
    /// Most significant bit first
    MSBFirst,
    /// Least significant bit first
    LSBFirst,
}

impl Default for BitOrder {
    /// Default bit order: MSB first
    fn default() -> Self {
        BitOrder::MSBFirst
    }
}

/// A Full-Duplex SPI implementation, takes 3 pins, and a timer running at 2x
/// the desired SPI frequency.
pub struct Spi<Miso, Mosi, Sck, Cs, Delay>
where
    Miso: InputPin,
    Mosi: OutputPin,
    Sck: OutputPin,
    Cs: OutputPin,
    Delay: DelayNs,
{
    mode: Mode,
    miso: Miso,
    mosi: Mosi,
    sck: Sck,
    cs: Cs,
    delay: Delay,
    delay_ns: u32,
    read_val: Option<u8>,
    bit_order: BitOrder,
}

impl<Miso, Mosi, Sck, Cs, Delay, E> Spi<Miso, Mosi, Sck, Cs, Delay>
where
    Miso: InputPin<Error = E>,
    Mosi: OutputPin<Error = E>,
    Sck: OutputPin<Error = E>,
    Cs: OutputPin<Error = E>,
    E: Debug + Display,
    Delay: DelayNs,
{
    /// Create instance
    pub fn build(
        mode: Mode,
        miso: Miso,
        mosi: Mosi,
        sck: Sck,
        cs: Cs,
        delay: Delay,
    ) -> Result<Self, Error<E>> {
        let mut spi = Spi {
            mode,
            miso,
            mosi,
            sck,
            cs,
            delay,
            delay_ns: 0,
            read_val: None,
            bit_order: BitOrder::default(),
        };

        match mode.polarity {
            Polarity::IdleLow => spi.sck.set_low().map_err(Error::Bus)?,
            Polarity::IdleHigh => spi.sck.set_high().map_err(Error::Bus)?,
        }

        Ok(spi)
    }

    /// `build`s with a pre-set `delay_ns`
    pub fn with_delay_ns(mut self, delay: u32) -> Self {
        self.delay_ns = delay;
        self
    }

    /// Set transmission bit order
    pub fn set_bit_order(&mut self, order: BitOrder) {
        self.bit_order = order;
    }

    /// Change the delay used by `wait_for_timer`
    pub fn set_delay_ns(&mut self, delay: u32) {
        self.delay_ns = delay;
    }

    fn read_bit(&mut self) -> Result<(), crate::spi::Error<E>> {
        let is_miso_high = self.miso.is_high().map_err(Error::Bus)?;
        let shifted_value = self.read_val.unwrap_or(0) << 1;
        if is_miso_high {
            self.read_val = Some(shifted_value | 1);
        } else {
            self.read_val = Some(shifted_value);
        }
        Ok(())
    }

    fn read_byte(&mut self) -> Result<u8, crate::spi::Error<E>> {
        self.read_val = Some(0);

        for _ in 0..8 {
            self.churn()?;
        }

        let result = match self.bit_order {
            BitOrder::MSBFirst => self.read_val.unwrap_or(0),
            BitOrder::LSBFirst => self.read_val.unwrap_or(0).reverse_bits(),
        };

        Ok(result)
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), crate::spi::Error<E>> {
        for bit_offset in 0..8 {
            let out_bit = match self.bit_order {
                BitOrder::MSBFirst => (byte >> (7 - bit_offset)) & 0b1,
                BitOrder::LSBFirst => (byte >> bit_offset) & 0b1,
            };

            if out_bit == 1 {
                self.mosi.set_high().map_err(Error::Bus)?;
            } else {
                self.mosi.set_low().map_err(Error::Bus)?;
            }

            self.churn()?;
        }

        Ok(())
    }

    fn churn(&mut self) -> Result<(), crate::spi::Error<E>> {
        match self.mode {
            MODE_0 => {
                self.wait_for_timer();
                self.set_clk_high()?;
                self.read_bit()?;
                self.wait_for_timer();
                self.set_clk_low()?;
            }
            MODE_1 => {
                self.set_clk_high()?;
                self.wait_for_timer();
                self.read_bit()?;
                self.set_clk_low()?;
                self.wait_for_timer();
            }
            MODE_2 => {
                self.wait_for_timer();
                self.set_clk_low()?;
                self.read_bit()?;
                self.wait_for_timer();
                self.set_clk_high()?;
            }
            MODE_3 => {
                self.set_clk_low()?;
                self.wait_for_timer();
                self.read_bit()?;
                self.set_clk_high()?;
                self.wait_for_timer();
            }
        }
        Ok(())
    }

    #[inline]
    fn set_clk_high(&mut self) -> Result<(), crate::spi::Error<E>> {
        self.sck.set_high().map_err(Error::Bus)
    }

    #[inline]
    fn set_clk_low(&mut self) -> Result<(), crate::spi::Error<E>> {
        self.sck.set_low().map_err(Error::Bus)
    }

    #[inline]
    fn wait_for_timer(&mut self) {
        self.delay.delay_ns(self.delay_ns);
    }
}

// TODO: impl SpiBus as well to allow for multiple devices.

impl<Miso, Mosi, Sck, Cs, Delay, E> SpiDevice<u8> for Spi<Miso, Mosi, Sck, Cs, Delay>
where
    Miso: InputPin<Error = E>,
    Mosi: OutputPin<Error = E>,
    Sck: OutputPin<Error = E>,
    Cs: OutputPin<Error = E>,
    E: Debug + Display,
    Delay: DelayNs,
{
    fn transaction(
        &mut self,
        operations: &mut [embedded_hal::spi::Operation<'_, u8>],
    ) -> Result<(), Self::Error> {
        self.cs.set_low().map_err(Error::Bus)?;
        self.wait_for_timer();

        use embedded_hal::spi::Operation::*;
        for op in operations {
            match op {
                DelayNs(ns) => self.delay.delay_ns(*ns),
                Read(miso) => {
                    for byte in miso.iter_mut() {
                        *byte = self.read_byte()?;
                    }
                }
                Write(mosi) => {
                    for byte in mosi.iter() {
                        self.write_byte(*byte)?
                    }
                }
                Transfer(_miso, _mosi) => {
                    unimplemented!()
                    // for (read_byte, write_byte) in read_from.iter_mut().zip(write_to.iter()) {
                    //     self.write_byte(*write_byte)?;
                    //     *read_byte = self.read_byte()?;
                    // }
                }
                TransferInPlace(buf) => {
                    for byte in buf.iter_mut() {
                        let sent_byte = *byte;
                        self.write_byte(sent_byte)?;
                        *byte = self.read_val.expect("Reply buffer wasn't filled?");
                    }
                }
            }
        }

        self.wait_for_timer();
        self.cs.set_high().map_err(Error::Bus)?;

        Ok(())
    }
}

impl<Miso, Mosi, Sck, Cs, Delay, E> ErrorType for Spi<Miso, Mosi, Sck, Cs, Delay>
where
    Miso: InputPin<Error = E>,
    Mosi: OutputPin<Error = E>,
    Sck: OutputPin<Error = E>,
    Cs: OutputPin<Error = E>,
    E: Debug + Display,
    Delay: DelayNs,
{
    type Error = Error<E>;
}
