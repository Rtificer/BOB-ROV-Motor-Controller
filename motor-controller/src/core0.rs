use embassy_rp::gpio::Input;
use embassy_rp::peripherals::{PIO0, PIO1};
use rp2040_dshot::encoder::Command as DShotCommand;
use rp2040_dshot::encoder::{StandardDShotVariant, DShotVariant};
use rp2040_dshot::driver::{StandardDShotDriver, DShotDriver};
use defmt::{error, info, warn};
use embassy_rp::pac as pac;

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
            driver.write_command(command, true).await.unwrap_or_else(|err| {
                error!("Error while writing DShot command to PIOs. Error: {}", err);
            });
        });
    } else {
        // Handle as throttle
        let raw = u16::from_le_bytes([buffer[0], buffer[1]]);

        let Some(throttle) = raw.checked_sub(raw - 48) else {
            error!("Invalid raw value: {}", raw);
            return;
        };

        for_each_driver!(sms, |driver| {
            driver.write_throttle(throttle, true).await.unwrap_or_else(|err| {
                error!("Error while writing Dshot throttle to PIOs. Error {}", err);
            });
        });
    }
}

#[embassy_executor::task]
pub async fn spi_task(
    mut cs_pin: Input<'static>,
    mut sms: SmDriverBatch
) {
    info!("Spawned core0 executor and spi task!");
    
    let mut transfer_buffer = [0u8; 2];

    // Initialize telemetry buffers
    let mut telemetry_byte_idx = 0;
    let mut telemetry_buffer = [0u8; 10];
    TELEMETRY_BUFFERS.read(&mut telemetry_buffer);

    let mut synced_count = 0;
    
    // Sync before sending telemetry to ensure simultanous exchange
    loop {
        cs_pin.wait_for_falling_edge().await;

        read(&mut transfer_buffer);

        let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
        let received_crc = transfer_buffer[1] & 0x0F;       

        if computed_crc == received_crc {
            if synced_count >= crate::spi::SYNC_THRESHOLD { 
                info!("Successfully synced spi!");
                break; 
            }

            synced_count += 1;
        } else {
            wait_one_transmission();
            synced_count = 0;
        }
    }

    loop {
        // Have we written all 10 bytes?
        if telemetry_byte_idx == 10 {
            TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
            // info!("Read {} from telemetry buffers", telemetry_buffer);
            telemetry_byte_idx = 0;
        }

        // Copy the next word into the transfer buffer.
        transfer_buffer = telemetry_buffer[telemetry_byte_idx..telemetry_byte_idx+2].try_into().expect("Telemetry buffer failed to copy into SPI transfer buffer!");

        // Write that data, and read in the next command.
        transfer_in_place(&mut transfer_buffer);

        telemetry_byte_idx += 2;

        let computed_crc = StandardDShotVariant::compute_crc(u16::from_le_bytes(transfer_buffer));
        let received_crc = transfer_buffer[1] & 0x0F;       

        if computed_crc != received_crc {
            warn!("CRC command missmatch. Expected {:04b}, got {:04b}. Invalid command frame: {:08b}", computed_crc, received_crc, transfer_buffer);
            continue;
        }

        info!("Read the following command: {:08b}", transfer_buffer);
        // Write the incoming DSHOT command.
        write_dshot(&mut sms, transfer_buffer).await;
    }
}

#[allow(clippy::cast_possible_truncation)]
fn transfer_in_place(transfer_buf: &mut [u8]) {
    for byte in transfer_buf {
        while tx_fifo_is_full() {} // Wait until tx FIFO is empty
        set_fifo(u16::from(*byte));
        while rx_fifo_is_empty() {} // Wait until rx FIFO is full
        *byte = get_fifo_data() as u8;
    }
    flush();
}

#[allow(clippy::cast_possible_truncation)]
fn read(read_buf: &mut [u8]) {
    for byte in read_buf {
        while tx_fifo_is_full() {} // Wait until tx FIFO is empty
        set_fifo(0);
        while rx_fifo_is_empty() {} // Wait until rx FIFO is full
        *byte = get_fifo_data() as u8;
    }
    flush();
}

fn wait_one_transmission() {
    while tx_fifo_is_full() {} // Wait until tx FIFO is empty
    set_fifo(0);
    while rx_fifo_is_empty() {} // Wait until rx FIFO is full
    flush();
}

#[inline(always)]
fn tx_fifo_is_full() -> bool {
    !pac::SPI0.sr().read().tnf()
}

#[inline(always)]
fn rx_fifo_is_empty() -> bool {
    !pac::SPI0.sr().read().rne()
}

#[inline(always)]
fn set_fifo(data: u16) {
    pac::SPI0.dr().write(|w| w.set_data(data));
}

#[inline(always)]
fn get_fifo_data() -> u16 {
    pac::SPI0.dr().read().data()
}

#[inline(always)]
fn flush() {
    // Wait until SSP is idle
    while pac::SPI0.sr().read().bsy() {}
}