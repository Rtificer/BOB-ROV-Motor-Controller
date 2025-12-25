//! A crate for using the rp2040's PIO machines to write DSHOT commands
//! 
//! Has support for both regular and BDDshot, as well as ESC telemetry, and extended BDDshot telemetry frames
//! VERY ALPHA, BDDShot features have not been tested and likely do not work.
#![no_std]

#[cfg(feature = "driver")]
pub mod program;
#[cfg(feature = "driver")]
pub use program::StandardDShotTimings as StandardDShotTimings;
#[cfg(feature = "driver")]
pub use program::BdDShotTimings as BdDShotTimings;
#[cfg(feature = "driver")]
pub mod driver;

mod encoder;

#[derive(Debug, Clone)]
#[cfg_attr(feature = "thiserror", derive(thiserror_no_std::Error))]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Error {
    /// Throttle command must be in the range 0-1999
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Throttle command must be in the range 0-1999, was {throttle}"))]
    ThrottleBoundsError { throttle: u16 },
    /// Clock divider conversion from float to fixed failed in clock divisor calculation.
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Conversion from float to fixed failed in clock divider calculation. Float value: {}", clock_divider_float))]
    ClockDividerConversionError { clock_divider_float: f32 },
    /// Failed to spawn task.
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Failed to spawn task"))]
    SpawnError(embassy_executor::SpawnError),
    /// Failed to recieve value from channel.
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Failed to recieve value from channel"))]
    TryReceiveError(embassy_sync::channel::TryReceiveError),
    /// Driver future timed out
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Driver future timed out"))]
    TimeoutError(embassy_time::TimeoutError),
    /// Invalid ERPM telemerty checksum.
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("Invalid ERPM telemetry checksum"))]
    InvalidTelemetryChecksum,
    /// TX Push Faliure
    #[cfg(feature = "driver")]
    #[cfg_attr(feature = "thiserror", error("TX Push Faliure"))]
    TxTryPushFaliure,
    #[cfg(feature = "driver")]
    /// State machine split faliure, empty pointer!
    #[cfg_attr(feature = "thiserror", error("SM Split Faliure, empty pointer!"))]
    SmSplitFaliure,
}

#[cfg(feature = "driver")]
impl From<embassy_executor::SpawnError> for Error {
    fn from(e: embassy_executor::SpawnError) -> Self {
        Error::SpawnError(e)
    }
}

#[cfg(feature = "driver")]
impl From<embassy_sync::channel::TryReceiveError> for Error {
    fn from(e: embassy_sync::channel::TryReceiveError) -> Self {
        Error::TryReceiveError(e)
    }
}

#[cfg(feature = "driver")]
impl From<embassy_time::TimeoutError> for Error {
    fn from(e: embassy_time::TimeoutError) -> Self {
        Error::TimeoutError(e)
    }
}