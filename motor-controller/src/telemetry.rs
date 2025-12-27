use static_assertions::assert_impl_all as assert_impl;
use embassy_rp::uart;

#[allow(clippy::wildcard_imports)]
use embassy_rp::peripherals::*;


pub fn get_uart_config() -> uart::Config {
    let mut config = uart::Config::default();

    // As per KISS ESC specfiication
    config.baudrate = 115_200; 
    config.data_bits = uart::DataBits::DataBits8;
    config.stop_bits = uart::StopBits::STOP1;
    config.parity = uart::Parity::ParityNone;

    config
}

macro_rules! define_telemetry_config {
    (
        rx_peripheral: $uart_rx:ty,
        rx_telemetry_pin: $rx_pin:ty,
        rx_dma_channel: $dma_channel_rx: ty,
        tx_peripheral: $uart_tx:ty,
        tx_telemetry_pin: $tx_pin:ty,
        tx_dma_channel: $dma_channel_tx:ty
    ) => {
        // Assert that given telemetry pin(s) is valid
        assert_impl!($rx_pin: uart::RxPin<$uart_rx>);
        assert_impl!($tx_pin: uart::TxPin<$uart_tx>);

        #[cfg(not(feature = "dummy-telemetry"))]
        #[macro_export]
        macro_rules! get_telemetry_peripherals {
            ($peripherals:ident) => {
                ::pastey::paste!{ ($peripherals.[<$uart_rx>], $peripherals.[<$rx_pin>], $peripherals.[<$dma_channel_rx>]) }
            }
        }

        #[cfg(feature = "dummy-telemetry")]
        #[macro_export]
        macro_rules! get_telemetry_peripherals {
            ($peripherals:ident) => {
                ::pastey::paste!{(
                    $peripherals.[<$uart_rx>], $peripherals.[<$rx_pin>], $peripherals.[<$dma_channel_rx>],
                    $peripherals.[<$uart_tx>], $peripherals.[<$tx_pin>], $peripherals.[<$dma_channel_tx>],
                )}
            }
        }

        /// Binds the UART interrupt corresponding to the provided `uart_rx`peripheral.
        #[macro_export]
        macro_rules! bind_telemetry_interrupt {
            () => {
                ::pastey::paste! { 
                    ::embassy_rp::bind_interrupts!(struct UartIrq {
                        [<$uart_rx _IRQ>] => ::embassy_rp::uart::InterruptHandler<::embassy_rp::peripherals::$uart_rx>;
                    });
                }
            }
        }
    };
}

define_telemetry_config! {
    rx_peripheral: UART1,
    rx_telemetry_pin: PIN_5,
    rx_dma_channel: DMA_CH0,

    // The following three are only used when dummy telemetry feature is enabled
    tx_peripheral: UART0,
    tx_telemetry_pin: PIN_12,
    tx_dma_channel: DMA_CH1
}