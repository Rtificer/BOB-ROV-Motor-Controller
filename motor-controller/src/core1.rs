use embassy_rp::uart::{self, UartRx};
use defmt::{error, info, warn};
use rp2040_dshot::encoder::TelemetryFrame;
use crate::TELEMETRY_BUFFERS;

#[derive(Clone, Copy, PartialEq)]
enum CrcState {
    /// The latest CRC checksum was valid.
    Valid,
    /// The latest CRC checksum was invalid
    Invalid
}

macro_rules! impl_dshot_telemetry_task {
    ($mode:ty, $read_fn:ident) => {
        #[embassy_executor::task]
        pub async fn dshot_telemetry_task(mut uart: UartRx<'static, $mode>) {
            info!("Spawned Core1 and telemetry executory!");
            
            let mut internal_buf = [0u8; 10];
            let mut crc_state = CrcState::Valid;

            info!("Reading DShot Telemetry...");
            loop {
                if let Err(read_error) = $read_fn(&mut uart, &mut internal_buf).await {
                    handle_uart_error(read_error);
                    continue;
                }

                // info!("Telemetry data: {:?}", internal_buf);

                let computed_crc = TelemetryFrame::compute_crc(&internal_buf[..9]);
                let received_crc = internal_buf[9];

                if internal_buf[9] != computed_crc {
                    warn!("Telemetry CRC mismatch! Expected {:08b}, got {:08b}. Attempting shift by one! Invalid telemetry frame: {}", computed_crc, received_crc, internal_buf);

                    let mut single_byte = [0u8; 1];
                    if let Err(read_error) = $read_fn(&mut uart, &mut single_byte).await {
                        handle_uart_error(read_error);
                    }

                    crc_state = CrcState::Invalid;
                    continue;
                }
                
                if crc_state == CrcState::Invalid {
                    info!("Success! Valid telemetry frame: {:?}", internal_buf);
                    crc_state = CrcState::Valid;
                }

                TELEMETRY_BUFFERS.write(&mut internal_buf);
                // info!("Wrote {:?} into telemetry buffer!", internal_buf);
            }
        }
    };
}

#[cfg(not(feature = "dummy-telemetry"))]
impl_dshot_telemetry_task!(uart::Blocking, blocking_read_async);

#[cfg(feature = "dummy-telemetry")]
impl_dshot_telemetry_task!(uart::Async, async_read_async);

// Small async wrappers to allow for macro definition 
// (maybe more code than copy+pasting dshot telemetry task at this point, but it does have a centralizing advantage)
// always inlined so should be 0 overhead.
#[cfg(not(feature = "dummy-telemetry"))]
#[inline(always)]
async fn blocking_read_async(
    uart: &mut UartRx<'static, uart::Blocking>, 
    buffer: &mut [u8]
) -> Result<(), uart::Error> {
    uart.blocking_read(buffer)
}

#[cfg(feature = "dummy-telemetry")]
#[inline(always)]
async fn async_read_async(
    uart: &mut UartRx<'static, uart::Async>,
    buf: &mut [u8],
) -> Result<(), uart::Error> {
    uart.read(buf).await
}

#[cfg(feature = "dummy-telemetry")]
#[embassy_executor::task]
pub async fn dummy_telemetry_writer(mut tx: uart::UartTx<'static, uart::Async>) {
    use rp2040_dshot::encoder::TelemetryFrame;
    use embassy_time::{Ticker, Duration};

    let mut data = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    let crc = TelemetryFrame::compute_crc(&data[..9]);
    info!("CRC cheksum: {}", crc);
    data[9] = crc;

    info!("Data to be sent from dummy telemetry writer: {}", data);

    // 115200 baud / 10 bytes / 10 bits per byte = ~868us. 1000 allows for a good saftey margin.
    let frame_period = Duration::from_micros(1000);
    let mut ticker = Ticker::every(frame_period);

    info!("Writing dummy DShot telemetry...");
    loop {
        ticker.next().await; // Wait until next tick

        if let Err(write_error) = tx.write(&data).await {
            handle_uart_error(write_error);
        }
    }
}

fn handle_uart_error(err: uart::Error) {
    match err {
        uart::Error::Overrun => error!("UART telemetry FIFO or shift-register overflowed!"),
        uart::Error::Break => error!("UART telemetry recieved erroneous break instruction!"),
        uart::Error::Framing => error!("UART telemetry failed to recieve a valid stop bit!"),
        // This should never happen bc/ KISS ESC protocol has 0 parity bits; see config.rs
        uart::Error::Parity => error!("UART telemetry packet parity detected error!"),
        _ => error!("Unknown UART telemetry error!")
    }
}