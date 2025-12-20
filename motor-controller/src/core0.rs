use embassy_rp::peripherals::{PIO0, PIO1};
use embassy_rp::spi::{self, Spi};
use rp2040_dshot::encoder::Command as DShotCommand;
use rp2040_dshot::encoder::{StandardDShotVariant, DShotVariant};
use rp2040_dshot::driver::{StandardDShotDriver, DShotDriver};
use embassy_time::TimeoutError;
use defmt::{error, info, warn};
// use embassy_rp::i2c_slave::{self, I2cSlave};
// use embassy_rp::i2c;
use embassy_time::{Timer, Duration};

use crate::config;
use crate::TELEMETRY_BUFFERS;
// #[cfg(feature = "dummy-spi")]
// use crate::config::spi::DummySpiPeripheral;
// use crate::config::spi::SpiPeripheral;


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


#[macro_export]
macro_rules! create_sm_driver_batch {
    ($pio0:ident, $pio1:ident) => {
        core0::SmDriverBatch {
            pio0_sm0: StandardDShotDriver::new($pio0.sm0),
            pio0_sm1: StandardDShotDriver::new($pio0.sm1),
            pio0_sm2: StandardDShotDriver::new($pio0.sm2),
            pio0_sm3: StandardDShotDriver::new($pio0.sm3),
            pio1_sm0: StandardDShotDriver::new($pio1.sm0),
            pio1_sm1: StandardDShotDriver::new($pio1.sm1),
            pio1_sm2: StandardDShotDriver::new($pio1.sm2),
            pio1_sm3: StandardDShotDriver::new($pio1.sm3),
        }
    };
}

macro_rules! for_each_driver {
    ($batch: expr, |$driver: ident| $body:expr) => {{
        let $driver = &mut $batch.pio0_sm0; $body;
        let $driver = &mut $batch.pio0_sm1; $body;
        let $driver = &mut $batch.pio0_sm2; $body;
        let $driver = &mut $batch.pio0_sm3; $body;
        let $driver = &mut $batch.pio1_sm0; $body;
        let $driver = &mut $batch.pio1_sm1; $body;
        let $driver = &mut $batch.pio1_sm2; $body;
        let $driver = &mut $batch.pio1_sm3; $body;
    }};
}


async fn write_dshot(sms: &mut SmDriverBatch, buffer: [u8; 2]) {
    let first_byte = buffer[0];

    if let Ok(command) = DShotCommand::try_from(first_byte) {
        // Handle as command
        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_command(command, true).await {
                // Match expression here to ensure all error variants are handled, (its 1, but ensure this throws an error if the error type is updated)
                match write_error {
                    TimeoutError => error!("Write Dshot command timeout error!"),
                }
            }
        });
    } else {
        // Handle as throttle
        let raw = u16::from_le_bytes([buffer[0], buffer[1]]);

        let Some(throttle) = raw.checked_sub(raw - 48) else {
            error!("Invalid raw value: {}", raw);
            return;
        };

        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_throttle(throttle, true).await {
                handle_dshot_write_error(&write_error);
            }
        });
    }
}


// fn handle_respond_to_read_result(result: Result<i2c_slave::ReadStatus, i2c_slave::Error>) {
//     match result {
//         Ok(i2c_slave::ReadStatus::Done) => (),
//         Ok(i2c_slave::ReadStatus::NeedMoreBytes) => warn!("i2c telemetry read failed: Controller attempted to read more bytes than provided!"),
//         Ok(i2c_slave::ReadStatus::LeftoverBytes(count)) => warn!("i2c telemetry read failed: Controller stopped reading with {} leftover bytes!", count),
//         Err(error) => handle_i2c_error(error),
//     }
// }

// fn handle_i2c_error(error: i2c_slave::Error) {
//     match error {
//         i2c_slave::Error::Abort(reason) => {
//             match reason {
//                 i2c::AbortReason::NoAcknowledge => error!("I2C Aborted! Bus operation not acknowledged!"),
//                 i2c::AbortReason::ArbitrationLoss => error!("I2C Aborted! Arbitration lost!"),
//                 i2c::AbortReason::TxNotEmpty(remaining) => error!("I2C Aborted! Transmit ended with data still in fifo! Remaining: {}", remaining),
//                 i2c::AbortReason::Other(data) => error!("I2C aborted! Reason unknown! Attached Data (idk what this is): {}", data)
//             }
//         }
//         i2c_slave::Error::InvalidResponseBufferLength => error!("I2C read buffer is 0 length!"),
//         _ => error!("Unknown i2c_slave error!")
//     }
// }

fn handle_dshot_write_error(error: &rp2040_dshot::Error) {
    match error {
        rp2040_dshot::Error::ThrottleBoundsError { throttle } => error!("DShot throttle bounds error! throttle: {}", throttle),
        rp2040_dshot::Error::TimeoutError(_) => error!("Write DShot throttle timeout error!"),
        _ => error!("Unknown write DShot throttle error!"),
    }
}


// #[embassy_executor::task]
// pub async fn i2c_task(
//     mut device: I2cSlave<'static, config::i2c::I2cPeripheral>,
//     mut sms: SmDriverBatch,
// ) {
//     info!("Spawned core0 executor and telemetry task!");
    
//     let mut write_buffer = [0u8; 2];
//     let mut telemetry_buffer = [0u8; 10];

//     loop {
//         match device.listen(&mut write_buffer).await {
//             Ok(i2c_slave::Command::Write(_)) => {
//                 write_dshot(&mut sms, write_buffer).await;
//             }
//             Ok(i2c_slave::Command::Read) => {
//                 TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
//                 handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
//             }
//             Ok(i2c_slave::Command::WriteRead(_)) => {
//                 write_dshot(&mut sms, write_buffer).await;
                
//                 TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
//                 handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
//             },
//             Ok(i2c_slave::Command::GeneralCall(_)) => warn!("Received erroneous GeneralCall i2c instruction!"),
//             Err(error) => handle_i2c_error(error),
//         }
//     }
// }

#[embassy_executor::task]
pub async fn spi_task(
    mut device: Spi<'static, config::spi::SpiPeripheral, spi::Blocking>,
    mut sms: SmDriverBatch
) {
    info!("Spawned core0 executor and spi task!");
    
    let mut transfer_buffer = [0u8; 2];
    // Initialize telemetry buffers
    let mut telemetry_buffer = [0u8; 10];
    TELEMETRY_BUFFERS.read(&mut telemetry_buffer);

    let mut telemetry_byte_idx = 0;

    let mut synced_count = 0;

    // Sync before sending telemetry to ensure simultanous exchange
    loop {
        if let Err(spi_err) = device.blocking_read(&mut transfer_buffer) {
            // No error varients are implemented by embassy_rp::spi, make this explicit
            error!("SPI transfer error {:?}", spi_err);
            continue;                  
        }

        let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
        let received_crc = telemetry_buffer[1] & 0x0F;       

        if computed_crc == received_crc {
            if synced_count >= config::spi::SYNC_THRESHOLD { 
                info!("Successfully synced spi!");
                break; 
            }

            synced_count += 1;
        } else {
            // Wait for 1 bit of time;
            Timer::after(Duration::from_hz(config::spi::FREQUENCY)).await;
            synced_count = 0;
        }
    }

    loop {
        // Have we written all 10 bytes?
        if telemetry_byte_idx == 10 {
            TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
            info!("Read {} from telemetry buffers", telemetry_buffer);
            telemetry_byte_idx = 0;
        }

        // Copy the next word into the transfer buffer.
        transfer_buffer = telemetry_buffer[telemetry_byte_idx..telemetry_byte_idx+2].try_into().expect("Telemetry buffer failed to copy into SPI transfer buffer!");

        // Write that data, and read in the next command.
        if let Err(spi_err) = device.blocking_transfer_in_place(&mut transfer_buffer) {
            // No error varients are implemented by embassy_rp::spi, make this explicit
            error!("SPI transfer error {:?}", spi_err);
            continue;
        }
        telemetry_byte_idx += 2;

        let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
        let received_crc = transfer_buffer[1] & 0x0F;       

        if computed_crc != received_crc {
            warn!("CRC command missmatch. Expected {:04b}, got {:04b}. Invalid command frame: {:08b}", computed_crc, received_crc, transfer_buffer);
            continue;
        }

        // Write the incoming DSHOT command.
        write_dshot(&mut sms, transfer_buffer).await;
    }
}

// macro_rules! impl_spi_task {
//     ($mode:ty, $read_fn:ident, $transfer_fn:ident) => {
//         #[embassy_executor::task]
//         pub async fn spi_task(
//             mut device: Spi<'static, config::spi::SpiPeripheral, $mode>,
//             mut sms: SmDriverBatch
//         ) {
//             info!("Spawned core0 executor and spi task!");
            
//             let mut transfer_buffer = [0u8; 2];
//             // Initialize telemetry buffers
//             let mut telemetry_buffer = [0u8; 10];
//             TELEMETRY_BUFFERS.read(&mut telemetry_buffer);

//             let mut telemetry_byte_idx = 0;

//             let mut synced_count = 0;

//             // Sync before sending telemetry to ensure simultanous exchange
//             loop {
//                 if let Err(spi_err) = $read_fn(&mut device, &mut transfer_buffer).await {
//                     // No error varients are implemented by embassy_rp::spi, make this explicit
//                     error!("SPI transfer error {:?}", spi_err);
//                     continue;                  
//                 };

//                 let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
//                 let received_crc = telemetry_buffer[1] & 0x0F;       

//                 if computed_crc == received_crc {
//                     if synced_count >= config::spi::SYNC_THRESHOLD { 
//                         info!("Succsefully synced spi!");
//                         break; 
//                     }

//                     synced_count += 1;
//                 } else {
//                     // Wait for 1 bit of time;
//                     Timer::after(Duration::from_hz(config::spi::FREQUENCY)).await;
//                     synced_count = 0;
//                 }
//             }

//             loop {
//                 // Have we written all 10 bytes?
//                 if telemetry_byte_idx == 10 {
//                     TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
//                     // info!("Read {} from telemetry buffers", telemetry_buffer);
//                     telemetry_byte_idx = 0;
//                 }

//                 // Copy the next word into the transfer buffer.
//                 transfer_buffer = telemetry_buffer[telemetry_byte_idx..telemetry_byte_idx+2].try_into().expect("Telemetry buffer failed to copy into SPI transfer buffer!");
//                 info!("current transfer buffer: {}", transfer_buffer);
//                 // Write that data, and read in the next command.
//                 if let Err(spi_err) = $transfer_fn(&mut device, &mut transfer_buffer).await {
//                     // No error varients are implemented by embassy_rp::spi, make this explicit
//                     error!("SPI transfer error {:?}", spi_err);
//                     continue;
//                 };
//                 telemetry_byte_idx += 2;

//                 let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
//                 let received_crc = transfer_buffer[1] & 0x0F;     

//                 if computed_crc != received_crc {
//                     warn!("CRC command missmatch. Expected {:04b}, got {:04b}. Invalid command frame: {:08b}", computed_crc, received_crc, transfer_buffer);
//                     continue;
//                 }

//                 // Write the incoming DSHOT command.
//                 write_dshot(&mut sms, transfer_buffer);

//                 info!("Standard loop iter");
//             }
//         }        
//     }
// }



// #[cfg(not(feature = "dummy-spi"))]
// impl_spi_task!(spi::Blocking, blocking_read_async, blocking_transfer_async);

// #[cfg(feature = "dummy-spi")]
// impl_spi_task!(spi::Async, async_read_async, async_transfer_async);

// // Small async wrappers to allow for macro definition 
// // (maybe more code than copy+pasting spi_task at this point, but it does have a centralizing advantage)
// // always inlined so should be 0 overhead.
// #[cfg(not(feature = "dummy-spi"))]
// #[inline(always)]
// async fn blocking_read_async(
//     spi: &mut Spi<'static, config::spi::SpiPeripheral, spi::Blocking>, 
//     buffer: &mut [u8]
// ) -> Result<(), spi::Error> {
//     spi.blocking_read(buffer)
// }

// #[cfg(feature = "dummy-spi")]
// #[inline(always)]
// async fn async_read_async(
//     spi: &mut Spi<'static, config::spi::SpiPeripheral, spi::Async>,
//     buf: &mut [u8],
// ) -> Result<(), spi::Error> {
//     spi.read(buf).await
// }

// #[cfg(not(feature = "dummy-spi"))]
// #[inline(always)]
// async fn blocking_transfer_async(
//     spi: &mut Spi<'static, config::spi::SpiPeripheral, spi::Blocking>, 
//     buffer: &mut [u8]
// ) -> Result<(), spi::Error> {
//     spi.blocking_transfer_in_place(buffer)
// }

// #[cfg(feature = "dummy-spi")]
// #[inline(always)]
// async fn async_transfer_async(
//     spi: &mut Spi<'static, config::spi::SpiPeripheral, spi::Async>,
//     buf: &mut [u8],
// ) -> Result<(), spi::Error> {
//     spi.transfer_in_place(buf).await
// }



// #[cfg(feature = "dummy-spi")]
// #[embassy_executor::task]
// pub async fn dummy_spi_controller(mut spi: Spi<'static, DummySpiPeripheral, spi::Async>) {
//     use rp2040_dshot::encoder::Frame;

//     info!("Started dummy_spi_controller task!");

//     let frame = Frame::<StandardDShotVariant>::from_throttle(1028, true).expect("Dummy frame construction failed!");

//     let frame_buffer = frame.inner().to_le_bytes(); 
//     info!("Dummy command frame buffer: {}", frame_buffer);
//     let mut transfer_buffer = [0u8; 2];

//     let mut telemetry_buffer = [0u8; 10];
//     let mut telemetry_byte_index = 0;


//     loop {
//         if telemetry_byte_index == 10 {
//             use rp2040_dshot::encoder::TelemetryFrame;

//             if let Some(telem_frame) = TelemetryFrame::from_bytes(&telemetry_buffer) {
//                 // info!("Read the the following telemetry data: {:?}", telem_frame);
//             } else {
//                 warn!("Failed to read create telemetry frame object!");
//             }

//             telemetry_byte_index = 0;
//         }

//         transfer_buffer = frame_buffer;

//         if let Err(spi_err) = spi.transfer_in_place(&mut transfer_buffer).await {
//             // No error varients are implemented by embassy_rp::spi, make this explicit
//             error!("SPI transfer error {:?}", spi_err);
//             continue;                  
//         }
//         telemetry_buffer[telemetry_byte_index..telemetry_byte_index+2].copy_from_slice(&transfer_buffer);

//         telemetry_byte_index += 2;

//         info!("dummy loop iter!");
//     }
// }