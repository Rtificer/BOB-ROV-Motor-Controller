#![no_std]
pub mod program;
pub mod encoder;
pub mod driver;

pub use program::StandardDShotTimings as StandardDShotTimings;
pub use program::BdDShotTimings as BdDShotTimings;

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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn generate_programs() {
//         let dshot1200 = program::generate_dshot_program!(1200_000, 12_000_000);
//     }
// }