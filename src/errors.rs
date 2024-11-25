pub type Result<T> = ::core::result::Result<T, AppError>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0:?}")]
    Display(display_interface::DisplayError),
    #[error(transparent)]
    Esp(#[from] esp_idf_sys::EspError),
    #[error(transparent)]
    Gpio(#[from] esp_idf_hal::gpio::GpioError),
    #[error(transparent)]
    StdIo(#[from] std::io::Error),
    #[error(transparent)]
    BitbangSpi(#[from] bitbang_hal::spi::Error<esp_idf_hal::gpio::GpioError>),
    #[error(transparent)]
    Postcard(#[from] postcard::Error),
}

impl From<display_interface::DisplayError> for AppError {
    fn from(value: display_interface::DisplayError) -> Self {
        Self::Display(value)
    }
}
