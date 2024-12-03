pub type Result<T> = ::core::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Display(String),
    #[error(transparent)]
    Esp(#[from] esp_idf_sys::EspError),
    #[error("{0}")]
    Bmp(String),
    #[error(transparent)]
    Gpio(#[from] esp_idf_hal::gpio::GpioError),
    #[error(transparent)]
    StdIo(#[from] std::io::Error),
    #[error(transparent)]
    BitbangSpi(#[from] bitbang_hal::spi::Error<esp_idf_hal::gpio::GpioError>),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
    #[error(transparent)]
    Ble(#[from] esp32_nimble::BLEError),
}

impl<SPI, DC> From<mipidsi::interface::SpiError<SPI, DC>> for AppError
where
    SPI: std::fmt::Debug,
    DC: std::fmt::Debug,
{
    fn from(value: mipidsi::interface::SpiError<SPI, DC>) -> Self {
        Self::Display(format!("{value:?}"))
    }
}

impl From<tinybmp::ParseError> for AppError {
    fn from(value: tinybmp::ParseError) -> Self {
        Self::Bmp(format!("{value:?}"))
    }
}

// impl From<display_interface::DisplayError> for AppError {
// fn from(value: display_interface::DisplayError) -> Self {
//     Self::Display(value)
// }
