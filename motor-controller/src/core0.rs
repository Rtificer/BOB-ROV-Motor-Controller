use embassy_rp::peripherals::*;
use rp2040_dshot::encoder::Command as DShotCommand;
use rp2040_dshot::driver::{StandardDShotDriver, DShotDriver};
use embassy_time::TimeoutError;
use defmt::{error, warn};
use embassy_rp::i2c_slave::{self, I2cSlave};
use embassy_rp::i2c;

use crate::config;
use crate::TELEMETRY_BUFFERS;

pub struct SmDriverBatch {
    pub pio0_sm0: StandardDShotDriver<'static, PIO0, 0>,
    pub pio0_sm1: StandardDShotDriver<'static, PIO0, 1>,
    pub pio0_sm2: StandardDShotDriver<'static, PIO0, 2>,
    pub pio0_sm3: StandardDShotDriver<'static, PIO0, 3>,
    pub pio1_sm0: StandardDShotDriver<'static, PIO1, 0>,
    pub pio1_sm1: StandardDShotDriver<'static, PIO1, 1>,
    pub pio1_sm2: StandardDShotDriver<'static, PIO1, 2>,
    pub pio1_sm3: StandardDShotDriver<'static, PIO1, 3>,
}



macro_rules! for_each_driver {
    ($batch: expr, |$driver: ident| $body:expr) => {{
        let $driver = &mut $batch.pio0_sm0;
        $body;
        let $driver = &mut $batch.pio0_sm1;
        $body;
        let $driver = &mut $batch.pio0_sm2;
        $body;
        let $driver = &mut $batch.pio0_sm3;
        $body;
        let $driver = &mut $batch.pio1_sm0;
        $body;
        let $driver = &mut $batch.pio1_sm1;
        $body;
        let $driver = &mut $batch.pio1_sm2;
        $body;
        let $driver = &mut $batch.pio1_sm3;
        $body;
    }};
}

async fn write_dshot(sms: &mut SmDriverBatch, buffer: &mut [u8; 2]) {
    let first_byte = buffer[0];

    if let Ok(command) = DShotCommand::try_from(first_byte) {
        // Handle as command
        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_command(command, true).await {
                // Match expression here to ensure all error varients are handled, (its 1, but ensure this throws an error if the error type is updated)
                match write_error {
                    TimeoutError => error!("Write Dshot command timeout error!"),
                }
                return;
            }
        })
    } else {
        // Handle as throttle
        let raw = u16::from_le_bytes([buffer[0], buffer[1]]);

        let Some(throttle) = raw.checked_sub(raw - 48) else {
            error!("Invalid raw value: {}", raw);
            return;
        };

        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_throttle(throttle, true).await {
                match write_error {
                    rp2040_dshot::Error::ThrottleBoundsError { throttle } => {
                        error!("DShot throttle bounds error! throttle: {}", throttle)
                    }
                    rp2040_dshot::Error::TimeoutError(_) => {
                        error!("Write DShot throttle timeout error!")
                    }
                    _ => error!("Unknown write DShot throttle error!"),
                }
                return;
            }
        })
    }
}

fn handle_respond_to_read_result(result: Result<i2c_slave::ReadStatus, i2c_slave::Error>) {
    match result {
        Ok(i2c_slave::ReadStatus::Done) => (),
        Ok(i2c_slave::ReadStatus::NeedMoreBytes) => warn!("i2c telemetry read failed: Controller attempted to read more bytes than provided!"),
        Ok(i2c_slave::ReadStatus::LeftoverBytes(count)) => warn!("i2c telemetry read failed: Controller stopped reading with {} leftover bytes!", count),
        Err(error) => handle_i2c_error(error),
    }
}

fn handle_i2c_error(error: i2c_slave::Error) {
    match error {
        i2c_slave::Error::Abort(reason) => {
            match reason {
                i2c::AbortReason::NoAcknowledge => error!("I2C Aborted! Bus operation not acknowledged!"),
                i2c::AbortReason::ArbitrationLoss => error!("I2C Aborted! Abritration lost!"),
                i2c::AbortReason::TxNotEmpty(remaining) => error!("I2C Aborted! Transmit ended with data still in fifo! Remaining: {}", remaining),
                i2c::AbortReason::Other(data) => error!("I2C aborted! Reason unknown! Attatched Data (idk what this is): {}", data)
            }
        }
        i2c_slave::Error::InvalidResponseBufferLength => error!("I2C read buffer is 0 length!"),
        _ => error!("Unknown i2c_slave error!")
    }
}

#[embassy_executor::task]
pub async fn i2c_task(
    mut device: I2cSlave<'static, config::i2c::I2cPeripheral>,
    mut sms: SmDriverBatch,
) {
    let mut write_buffer = [0u8; 2];
    let mut telemetry_buffer = [0u8; 10];

    loop {
        match device.listen(&mut write_buffer).await {
            Ok(i2c_slave::Command::Write(_)) => {
                write_dshot(&mut sms, &mut write_buffer).await;
            }
            Ok(i2c_slave::Command::Read) => {
                TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
                handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
            }
            Ok(i2c_slave::Command::WriteRead(_)) => {
                write_dshot(&mut sms, &mut write_buffer).await;
                
                TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
                handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
            },
            Ok(i2c_slave::Command::GeneralCall(_)) => warn!("Recieved erronious GeneralCall i2c instruction!"),
            Err(error) => handle_i2c_error(error),
        }
    }
}