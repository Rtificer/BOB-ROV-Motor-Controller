use embassy_rp::uart::{self, UartRx};
use defmt::{error};

use crate::TELEMETRY_BUFFERS;


#[embassy_executor::task]
pub async fn dshot_telemetry_task(mut uart: UartRx<'static, uart::Blocking>) {
    let mut internal_buf = [0u8; 10];

    loop {
        if let Err(read_error) = uart.blocking_read(&mut internal_buf) {
            match read_error {
                uart::Error::Overrun => error!("UART telemetry FIFO or shift-register overflowed!"),
                uart::Error::Break => error!("UART telemetry recieved erroneous break instruction!"),
                uart::Error::Framing => error!("UART telemetry failed to recieve a valid stop bit!"),
                // This should never happen bc/ KISS ESC protocol has 0 parity bits; see config.rs
                uart::Error::Parity => error!("UART telemetry packet parity detected error!"),
                _ => error!("Unknown UART telemetry error!")
            }
            continue;
        }

        TELEMETRY_BUFFERS.write(&mut internal_buf);
    }
}