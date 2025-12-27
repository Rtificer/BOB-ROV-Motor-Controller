use static_assertions::assert_impl_all as assert_impl;
use embassy_rp::pio::{self, Pio, PioPin, Pin, Instance, StateMachine};
use rp2040_dshot::encoder::DShotSpeed;
use embassy_rp::Peri;
use fixed::FixedU32;
use fixed::types::extra::U8;

#[allow(clippy::wildcard_imports)]
use embassy_rp::peripherals::*;

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