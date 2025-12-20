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


pub mod encoder;




#[derive(Debug, Clone, thiserror_no_std::Error)]
pub enum Error {
    /// Throttle command must be in the range 0-1999
    #[error("Throttle command must be in the range 0-1999, was {throttle}")]
    ThrottleBoundsError { throttle: u16 },
    /// Clock divider conversion from float to fixed failed in clock divisor calculation.
    #[error("Conversion from float to fixed failed in clock divider calculation. Float value: {}", clock_divider_float)]
    ClockDividerConversionError { clock_divider_float: f32 },
    /// Failed to spawn task.
    #[error("Failed to spawn task")]
    SpawnError(#[from] embassy_executor::SpawnError),
    /// Failed to recieve value from channel.
    #[error("Failed to recieve value from channel")]
    TryReceiveError(#[from] embassy_sync::channel::TryReceiveError),
    /// Driver future timed out
    #[error("Driver future timed out")]
    TimeoutError(#[from] embassy_time::TimeoutError),
    /// Invalid ERPM telemerty checksum.
    #[error("Invalid ERPM telemetry checksum")]
    InvalidTelemetryChecksum,
    /// TX Push Faliure
    #[error("TX Push Faliure")]
    TxTryPushFaliure,
    /// State machine split faliure, empty pointer!
    #[error("SM Split Faliure, empty pointer!")]
    SmSplitFaliure,
}

