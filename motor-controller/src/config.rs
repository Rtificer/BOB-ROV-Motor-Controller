pub mod i2c {
    use embassy_rp::i2c::SclPin as SclPinTrait;
    use embassy_rp::i2c::SdaPin as SdaPinTrait;
    use embassy_rp::i2c_slave;
    use embassy_rp::peripherals::I2C0;
    use embassy_rp::peripherals::{PIN_0, PIN_1};
    use embassy_rp::{Peri, Peripherals};
    use pastey::paste;
    use static_assertions::assert_impl_all as assert_impl;

    macro_rules! define_i2c_config {
        (
            peripheral: $i2c_peripheral:ty,
            scl_pin: $scl_pin:ty,
            sda_pin: $sda_pin:ty,
            addr: $addr:expr,
            general_call: $general_call:expr,
            scl_pullup: $scl_pullup:expr,
            sda_pullup: $sda_pullup:expr,
            buffer_size: $buffer_size:expr
        ) => {
            pub type I2cPeripheral = $i2c_peripheral;

            pub type SclPin = $scl_pin;
            pub type SdaPin = $sda_pin;

            // Asserts the types the given SLC pin and I2C Peripheral to implement SclPinTrait,
            assert_impl!(SclPin: SclPinTrait<I2cPeripheral>);

            // Asserts the types the given SDA pin and I2C instance to implement SdaPinTrait,
            assert_impl!(SdaPin: SdaPinTrait<I2cPeripheral>);

            /// Gets the correct peripherals based on configured I2C
            #[macro_export]
            macro_rules! get_i2c_peripherals {
                ($peripherals:ident) => {
                    pastey::paste! { (p.[<$i2c_peripheral>], p.[<$scl_pin>], p.[<$sda_pin>]) }
                }
            }

            /// Initilizes a new [`i2c_slave::Config`] object given the config values set in config module
            pub fn new() -> i2c_slave::Config {
                let mut config = i2c_slave::Config::default();
                config.addr = $addr;
                config.general_call = $general_call;
                config.scl_pullup = $scl_pullup;
                config.sda_pullup = $sda_pullup;

                config
            }

            pub const BUFFER_SIZE: usize = $buffer_size;
        };
    }

    define_i2c_config! {
        peripheral: I2C0,
        scl_pin: PIN_1,
        sda_pin: PIN_0,
        addr: 0x60,
        general_call: false,
        scl_pullup: false,
        sda_pullup: false,
        buffer_size: 128
    }
}

pub mod dshot {
    use static_assertions::assert_impl_all as assert_impl;
    use embassy_rp::peripherals::*;
    use embassy_rp::pio::{self, Pio, PioPin, Pin, Instance, StateMachine};
    use embassy_rp::Peripherals;
    use rp2040_dshot::encoder::DShotSpeed;
    use embassy_rp::Peri;
    use pastey::paste;
    use fixed::FixedU32;
    use fixed::types::extra::{U8, U4};

    

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
            pub type TopFrontRightPin = $top_front_right_pin;
            pub type TopFrontLeftPin = $top_front_left_pin;
            pub type TopBackRightPin = $top_back_right_pin;
            pub type TopBackLeftPin = $top_back_left_pin;
            pub type BottomFrontRightPin = $bottom_front_right_pin;
            pub type BottomFrontLeftPin = $bottom_front_left_pin;
            pub type BottomBackRightPin = $bottom_back_right_pin;
            pub type BottomBackLeftPin = $bottom_back_left_pin;

            // Ensure that all probided pins are valid.
            assert_impl!(TopFrontRightPin: PioPin);
            assert_impl!(TopFrontLeftPin: PioPin);
            assert_impl!(TopBackRightPin: PioPin);
            assert_impl!(TopBackLeftPin: PioPin);
            assert_impl!(BottomFrontRightPin: PioPin);
            assert_impl!(BottomFrontLeftPin: PioPin);
            assert_impl!(BottomBackRightPin: PioPin);
            assert_impl!(BottomBackLeftPin: PioPin);

            /// Gets the correct dshot pins as defined by define_dshot_config!
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
            
            pub fn set_pio_config<'d>
            (
                pio0: &mut Pio<'d, PIO0>, 
                pio1: &mut Pio<'d, PIO1>,
                top_front_right_pin: Peri<'d, TopFrontRightPin>,
                top_front_left_pin: Peri<'d, TopFrontLeftPin>,
                top_back_right_pin: Peri<'d, TopBackRightPin>,
                top_back_left_pin: Peri<'d, TopBackLeftPin>,
                bottom_front_right_pin: Peri<'d, BottomFrontRightPin>,
                bottom_front_left_pin: Peri<'d, BottomFrontLeftPin>,
                bottom_back_right_pin: Peri<'d, BottomBackRightPin>,
                bottom_back_left_pin: Peri<'d, BottomBackLeftPin>,
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
                config.clock_divider = PIO_CLOCK_DIVDER;

                config.set_set_pins(&[pin]);
                config.set_out_pins(&[pin]);

                sm.set_config(&config);
            }

            pub const DSHOT_SPEED: DShotSpeed = $dshot_speed;
            pub const PIO_CLOCK_HZ: u32 = $pio_clock;
            pub const UPDATE_RATE_HZ: u32 = $update_rate;

            pub const PIO_CLOCK_DIVDER: FixedU32<U8> = FixedU32::<U8>::from_bits(
                FixedU32::unwrapped_div(
                    FixedU32::<U4>::const_from_int(PIO_CLOCK_HZ),
                    FixedU32::<U4>::const_from_int(DSHOT_SPEED.bit_rate_hz())
                ).to_bits() >> 4
            );
        };
    }     

    define_dshot_config! {
        top_front_right_pin: PIN_2,
        top_front_left_pin: PIN_3,
        top_back_right_pin: PIN_4,
        top_back_left_pin: PIN_5,
        bottom_front_right_pin: PIN_6,
        bottom_front_left_pin: PIN_7,
        bottom_back_right_pin: PIN_8,
        bottom_back_left_pin: PIN_9,
        dshot_speed: DShotSpeed::DShot1200,
        pio_clock_hz: 19_200_000,
        update_rate_hz: 19_200_000
    }
}