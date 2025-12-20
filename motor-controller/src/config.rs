// pub mod i2c {
//     use embassy_rp::i2c::SclPin as SclPinTrait;
//     use embassy_rp::i2c::SdaPin as SdaPinTrait;
//     use embassy_rp::i2c_slave;
//     use embassy_rp::peripherals::*;
//     use static_assertions::assert_impl_all as assert_impl;

//     macro_rules! define_i2c_config {
//         (
//             peripheral: $i2c_peripheral:ty,
//             scl_pin: $scl_pin:ty,
//             sda_pin: $sda_pin:ty,
//             addr: $addr:expr,
//             general_call: $general_call:expr,
//             scl_pullup: $scl_pullup:expr,
//             sda_pullup: $sda_pullup:expr,
//         ) => {
//             // Asserts that the types of the given SLC pin, SDA, and I2C Peripheral are valid
//             assert_impl!($scl_pin: SclPinTrait<$i2c_peripheral>);
//             assert_impl!($sda_pin: SdaPinTrait<$i2c_peripheral>);

//             pub type I2cPeripheral = $i2c_peripheral;

//             /// Gets the correct peripherals based on configured I2C
//             #[macro_export]
//             macro_rules! get_i2c_peripherals {
//                 ($peripherals:ident) => {
//                     pastey::paste! { ($peripherals.[<$i2c_peripheral>], $peripherals.[<$scl_pin>], $peripherals.[<$sda_pin>]) }
//                 }
//             }
            
//             /// Binds the i2c interrupt corresponding to the provided `i2c_peripheral`
//             #[macro_export]
//             macro_rules! bind_i2c_interrupt {
//                 () => {
//                     pastey::paste! {
//                         bind_interrupts!(struct I2cIrq {
//                             [<$i2c_peripheral _IRQ>] => i2c::InterruptHandler<$i2c_peripheral>;
//                         });
//                     } 
//                 }
//             }

//             /// Initilizes a new [`i2c_slave::Config`] object given the config values set in config module
//             pub fn new() -> i2c_slave::Config {
//                 let mut config = i2c_slave::Config::default();
//                 config.addr = $addr;
//                 config.general_call = $general_call;
//                 config.scl_pullup = $scl_pullup;
//                 config.sda_pullup = $sda_pullup;

//                 config
//             }
//         };
//     }

//     define_i2c_config! {
//         peripheral: I2C0,
//         scl_pin: PIN_1,
//         sda_pin: PIN_0,
//         addr: 0x60,
//         general_call: false,
//         scl_pullup: false,
//         sda_pullup: false,
//     }
// }

pub mod spi {
    use embassy_rp::spi::{
        self,
        Phase, Polarity, 
        ClkPin, MosiPin, MisoPin
    };
    use embassy_rp::peripherals::*;
    use static_assertions::assert_impl_all as assert_impl;

    macro_rules! define_spi_config {
        (
            peripheral: $spi_peri:ty,
            clock_pin: $clk_pin:ty,
            mosi_pin: $mosi_pin:ty,
            miso_pin: $miso_pin:ty,
            frequency: $frequency:expr,
            phase: $phase:expr,
            polarity: $polarity:expr,
            sync_threshhold: $sync_threshold:expr,
            // dummy_spi_peripheral: $dummy_spi_peri:ty,
            // dummy_clock_pin: $dummy_clk_pin:ty,
            // dummy_mosi_pin: $dummy_mosi_pin:ty,
            // dummy_miso_pin: $dummy_miso_pin:ty,
            // rx_dma: $rx_dma:ty,
            // tx_dma: $tx_dma:ty,
            // dummy_rx_dma: $dummy_rx_dma:ty,
            // dummy_tx_dma: $dummy_tx_dma:ty
        ) => {
            assert_impl!($clk_pin: ClkPin<$spi_peri>);
            assert_impl!($mosi_pin: MosiPin<$spi_peri>);
            assert_impl!($miso_pin: MisoPin<$spi_peri>);

            // assert_impl!($dummy_clk_pin: ClkPin<$dummy_spi_peri>);
            // assert_impl!($dummy_mosi_pin: MosiPin<$dummy_spi_peri>);
            // assert_impl!($dummy_miso_pin: MisoPin<$dummy_spi_peri>);

            pub type SpiPeripheral = $spi_peri;
            // pub type DummySpiPeripheral = $dummy_spi_peri;

            /// Gets the correct peripherals based on the values configered in [`define_spi_config!`]
            // #[cfg(not(feature = "dummy-spi"))]
            #[macro_export]
            macro_rules! get_spi_peripherals {
                ($peripherals:ident) => {
                    ::pastey::paste!{ ($peripherals.[<$spi_peri>], $peripherals.[<$clk_pin>], $peripherals.[<$mosi_pin>], $peripherals.[<$miso_pin>]) }
                }
            }

            // #[cfg(feature = "dummy-spi")]
            // #[macro_export]
            // macro_rules! get_spi_peripherals {
            //     ($peripherals:ident) => {
            //         ::pastey::paste!{(
            //             $peripherals.[<$spi_peri>], $peripherals.[<$clk_pin>], $peripherals.[<$mosi_pin>], $peripherals.[<$miso_pin>], $peripherals.[<$rx_dma>], $peripherals.[<$tx_dma>],
            //             $peripherals.[<$dummy_spi_peri>], $peripherals.[<$dummy_clk_pin>], $peripherals.[<$dummy_mosi_pin>], $peripherals.[<$dummy_miso_pin>], $peripherals.[<$dummy_rx_dma>], $peripherals.[<$dummy_tx_dma>]
            //         )}
            //     }
            // }

            /// Initlizes a new [`spi::Config`] object given the values configered in [`define_spi_config!`]
            pub fn new() -> spi::Config {
                let mut config = spi::Config::default();
                config.frequency = $frequency;
                config.phase = $phase;
                config.polarity = $polarity;

                config
            }

            pub const FREQUENCY: u64 = $frequency;
            pub const SYNC_THRESHOLD: u8 = $sync_threshold;
        };
    }

    define_spi_config! {
        peripheral: SPI0,
        clock_pin: PIN_2,
        mosi_pin: PIN_3,
        miso_pin: PIN_4,
        frequency: 12_500_000,
        phase: Phase::CaptureOnFirstTransition,
        polarity: Polarity::IdleLow,
        sync_threshhold: 3,

        // // The following are only used when the dummy spi feature is enabled
        // dummy_spi_peripheral: SPI1,
        // dummy_clock_pin: PIN_10,
        // dummy_mosi_pin: PIN_11,
        // dummy_miso_pin: PIN_28,
        // rx_dma: DMA_CH2,
        // tx_dma: DMA_CH3,
        // dummy_rx_dma: DMA_CH4,
        // dummy_tx_dma: DMA_CH5
    }
}

pub mod dshot {
    use static_assertions::assert_impl_all as assert_impl;
    use embassy_rp::peripherals::*;
    use embassy_rp::pio::{self, Pio, PioPin, Pin, Instance, StateMachine};
    use rp2040_dshot::encoder::DShotSpeed;
    use embassy_rp::Peri;
    use fixed::FixedU32;
    use fixed::types::extra::U8;

    

    macro_rules! define_dshot_config {
        (
            top_front_right_pin: $top_front_right_pin:ty,
            top_front_left_pin: $top_front_left_pin:ty,
            top_back_right_pin: $top_back_right_pin:ty,
            top_back_left_pin: $top_back_left_pin:ty,
            bottom_front_right_pin: $bottom_front_right_pin:ty,
            bottom_front_left_pin: $bottom_front_left_pin:ty,
            bottom_back_right_pin: $bottom_back_right_pin:ty,
            bottom_back_left_pin: $bottom_back_left_pin:ty,
            dshot_speed: $dshot_speed:expr,
            pio_clock_hz: $pio_clock:expr,
            update_rate_hz: $update_rate:expr
        ) => {
            // Ensure that all provided pins are valid.
            assert_impl!($top_front_right_pin: PioPin);
            assert_impl!($top_front_left_pin: PioPin);
            assert_impl!($top_back_right_pin: PioPin);
            assert_impl!($top_back_left_pin: PioPin);
            assert_impl!($bottom_front_right_pin: PioPin);
            assert_impl!($bottom_front_left_pin: PioPin);
            assert_impl!($bottom_back_right_pin: PioPin);
            assert_impl!($bottom_back_left_pin: PioPin);


            /// Gets the correct dshot pins as defined by [`define_dshot_config!`]
            #[macro_export]
            macro_rules! get_dshot_pins {
                ($peripherals:ident) => {
                    pastey::paste! {(
                        $peripherals.[<$top_front_right_pin>],
                        $peripherals.[<$top_front_left_pin>],
                        $peripherals.[<$top_back_right_pin>],
                        $peripherals.[<$top_back_left_pin>],
                        $peripherals.[<$bottom_front_right_pin>],
                        $peripherals.[<$bottom_front_left_pin>],
                        $peripherals.[<$bottom_back_right_pin>],
                        $peripherals.[<$bottom_back_left_pin>],
                    )}
                }
            }
            
            #[allow(clippy::too_many_arguments)]
            pub fn set_pio_config<'d>
            (
                pio0: &mut Pio<'d, PIO0>, 
                pio1: &mut Pio<'d, PIO1>,
                top_front_right_pin: Peri<'d, $top_front_right_pin>,
                top_front_left_pin: Peri<'d, $top_front_left_pin>,
                top_back_right_pin: Peri<'d, $top_back_right_pin>,
                top_back_left_pin: Peri<'d, $top_back_left_pin>,
                bottom_front_right_pin: Peri<'d, $bottom_front_right_pin>,
                bottom_front_left_pin: Peri<'d, $bottom_front_left_pin>,
                bottom_back_right_pin: Peri<'d, $bottom_back_right_pin>,
                bottom_back_left_pin: Peri<'d, $bottom_back_left_pin>,
            ) {
                let top_front_right_pin = pio0.common.make_pio_pin(top_front_right_pin);
                let top_front_left_pin = pio0.common.make_pio_pin(top_front_left_pin);
                let top_back_right_pin = pio0.common.make_pio_pin(top_back_right_pin);
                let top_back_left_pin = pio0.common.make_pio_pin(top_back_left_pin);
                let bottom_front_right_pin = pio1.common.make_pio_pin(bottom_front_right_pin);
                let bottom_front_left_pin = pio1.common.make_pio_pin(bottom_front_left_pin);
                let bottom_back_right_pin = pio1.common.make_pio_pin(bottom_back_right_pin);
                let bottom_back_left_pin = pio1.common.make_pio_pin(bottom_back_left_pin);

                set_sm_config(&mut pio0.sm0, &top_front_right_pin);
                set_sm_config(&mut pio0.sm1, &top_front_left_pin);
                set_sm_config(&mut pio0.sm2, &top_back_right_pin);
                set_sm_config(&mut pio0.sm3, &top_back_left_pin);
                set_sm_config(&mut pio1.sm0, &bottom_front_right_pin);
                set_sm_config(&mut pio1.sm1, &bottom_front_left_pin);
                set_sm_config(&mut pio1.sm2, &bottom_back_right_pin);
                set_sm_config(&mut pio1.sm3, &bottom_back_left_pin);
            } 

            fn set_sm_config<'d, PIO: Instance, const SM: usize> (
                sm: &mut StateMachine<'d, PIO, SM>,
                pin: &Pin<'d, PIO>
            ) {
                let mut config = pio::Config::<PIO>::default();
                config.clock_divider = PIO_CLOCK_DIVIDER;

                config.set_set_pins(&[pin]);
                config.set_out_pins(&[pin]);

                sm.set_config(&config);
            }

            pub const DSHOT_SPEED: DShotSpeed = $dshot_speed;
            pub const PIO_CLOCK_HZ: u32 = $pio_clock;
            pub const UPDATE_RATE_HZ: u32 = $update_rate;

            pub const PIO_CLOCK_DIVIDER: FixedU32<U8> = FixedU32::unwrapped_div(
                FixedU32::<U8>::const_from_int(PIO_CLOCK_HZ),
                FixedU32::<U8>::const_from_int(DSHOT_SPEED.bit_rate_hz())
            );
        };
    }     

    define_dshot_config! {
        top_front_right_pin: PIN_13,
        top_front_left_pin: PIN_14,
        top_back_right_pin: PIN_15,
        top_back_left_pin: PIN_16,
        bottom_front_right_pin: PIN_17,
        bottom_front_left_pin: PIN_18,
        bottom_back_right_pin: PIN_19,
        bottom_back_left_pin: PIN_20,
        dshot_speed: DShotSpeed::DShot300,
        pio_clock_hz: 8_000_000,
        update_rate_hz: 8_000
    }
}

pub mod telemetry {
    use static_assertions::assert_impl_all as assert_impl;
    use embassy_rp::peripherals::*;
    use embassy_rp::uart;


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
}