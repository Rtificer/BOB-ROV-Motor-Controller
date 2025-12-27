use embassy_rp::pac as pac;
use pac::{SPI0, spi};
use embassy_rp::clocks;
use defmt::info;



macro_rules! define_spi_config {
    (
        peripheral: $spi_peri:expr,
        clock_pin: $clk_pin:expr,
        tx_pin: $tx_pin:expr,
        rx_pin: $rx_pin:expr,
        cs_pin: $cs_pin:ty,
        baud_rate: $baud_rate:expr,
        polarity: $polarity:expr,
        phase: $phase:expr,
        sync_threshhold: $sync_threshold:expr,
    ) => {
        pub const PERIPHERAL: spi::Spi = $spi_peri;

        pub const CLK_PIN: usize = $clk_pin;
        pub const TX_PIN: usize = $tx_pin;
        pub const RX_PIN: usize = $rx_pin;
        
        #[macro_export]
        macro_rules! get_spi_cs_pin {
            ($peripherals:ident) => {
                // Pull up to avoid floating line when not selected.
                ::pastey::paste! { ::embassy_rp::gpio::Input::new($peripherals.[<$cs_pin>], ::embassy_rp::gpio::Pull::Up) }
            } 
        }

        pub const POLARITY: bool = $polarity;
        pub const PHASE: bool = $phase;
        pub const BAUD_RATE: u32 = $baud_rate;
        pub const SYNC_THRESHOLD: u8 = $sync_threshold; 
    };
}

define_spi_config! {
    peripheral: SPI0,
    clock_pin: 2,
    tx_pin: 3,
    rx_pin: 4,
    cs_pin: PIN_6,
    baud_rate: 1_000_000,
    polarity: false,
    phase: false,
    sync_threshhold: 3,
}



pub fn configure() {
    // Reset peripheral
    pac::RESETS.reset().modify(|w| w.set_spi0(true));
    pac::RESETS.reset().modify(|w| w.set_spi0(false));
    while !pac::RESETS.reset_done().read().pio0() {};

    // Disable peripheral
    PERIPHERAL.cr1().write(|w| w.set_sse(false));

    // Set slave mode
    PERIPHERAL.cr1().write(|w| w.set_ms(true));

    PERIPHERAL.cr0().write(|w| {
        // Set 8 bit frame size (n - 1)
        w.set_dss(0b0111);
        // Set motorola mode
        w.set_frf(0b00);
        // Set polarity and phase values
        w.set_sph(PHASE);
        w.set_spo(POLARITY);
    });

    let achieved_baudrate = set_baudrate();
    info!("SPI baudrate set to {}", achieved_baudrate);

    configure_pin(CLK_PIN);
    configure_pin(RX_PIN);
    configure_pin(TX_PIN);
}


/// Set SPI baudrate
/// 
/// Returns the actual frequency achieved
pub fn set_baudrate() -> u32 {
    let mut prescale: u8 = u8::MAX;
    let mut postdiv: u8 = 0;

    let peri_frequency_hz = clocks::clk_peri_freq();

    // Find smallest prescale value which puts output frequency in range of
    // post-divide. Prescale is an even number from 2 to 254 inclusive.
    for prescale_option in (2u32..=254).step_by(2) {
        // We need to use an saturating_mul here because with a high baudrate certain invalid prescale
        // values might not fit in u32. However we can be sure those values exceed the max sys_clk frequency
        // So clamping a u32::MAX is fine here...
        #[allow(clippy::cast_possible_truncation)]
        if peri_frequency_hz < ((prescale_option + 2) * 256).saturating_mul(BAUD_RATE) {
            prescale = prescale_option as u8;
            break;
        }
    }

    // We might not find a prescale value that lowers the clock freq enough, so we leave it at max
    debug_assert_ne!(prescale, u8::MAX, "Could not find valid prescale value");

    // Find largest post-divide which makes output <= baudrate. 
    // Post-divide is an integer in the range 0 to 255 inclusive.
    for postdiv_option in (1..=255u8).rev() {
        if peri_frequency_hz / (u32::from(prescale) * u32::from(postdiv_option)) > BAUD_RATE {
            postdiv = postdiv_option;
            break;
        }
    }

    // Set prescale divisor (even number 2-254)
    PERIPHERAL.cpsr().write(|w| w.set_cpsdvsr(prescale));
    
    // Set serial clock rate (SCR) - this is the postdiv value
    PERIPHERAL.cr0().modify(|w| w.set_scr(postdiv));

    // Return the frequency we were able to achieve
    peri_frequency_hz / (u32::from(prescale) * (1 + u32::from(postdiv)))
}

fn configure_pin(pin: usize) {
    // Set as SPI Pin
    pac::IO_BANK0.gpio(pin).ctrl().write(|w| w.set_funcsel(1));

    pac::PADS_BANK0.gpio(pin).write(|w| {
        w.set_schmitt(false); // Enable schmitt filtering.
        w.set_slewfast(false); // Set slewrate to slow
        w.set_ie(true); // Enable Input
        w.set_od(false); // Enable Output
        w.set_pue(false); // Disable Pullup
        w.set_pde(false); // Disable Pulldown
    });
}