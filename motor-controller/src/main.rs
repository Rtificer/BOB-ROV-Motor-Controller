#![no_std]
#![no_main]

mod config;
mod core0;
mod core1;

use core::cell::UnsafeCell;
use core::ptr::addr_of_mut;
use core::sync::atomic::{AtomicU8, Ordering};
use embassy_executor::Executor;
use embassy_rp::clocks::ClockConfig;
use embassy_rp::config::Config as EmbassyConfig;
use embassy_rp::i2c;
use embassy_rp::i2c_slave::I2cSlave;
use embassy_rp::multicore::{Stack, spawn_core1};
use embassy_rp::peripherals::{I2C0, PIO0, PIO1};
use embassy_rp::pio::{self, Pio};
use embassy_rp::uart::{self, UartRx};
use embassy_rp::bind_interrupts;
use rp2040_dshot::StandardDShotTimings;
use rp2040_dshot::driver::StandardDShotDriver;
use rp2040_dshot::program::generate_standard_dshot_program;
use static_cell::StaticCell;

use panic_probe as _;
use defmt_rtt as _;

use crate::config::dshot::{DSHOT_SPEED, PIO_CLOCK_HZ, UPDATE_RATE_HZ};


static mut CORE1_STACK: Stack<4096> = Stack::new();
static CORE0_THREAD_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_THREAD_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static TELEMETRY_BUFFERS: DoubleBuffer = DoubleBuffer {
    buffers: UnsafeCell::new([[0u8; 10]; 2]),
    current: AtomicU8::new(0)
};


bind_interrupts!(struct PioIrqs {
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
});
bind_i2c_interrupt!();
bind_telemetry_interrupt!();


/// Double buffered telemetry so writer never blocks reader (vroom vroom)
struct DoubleBuffer {
    buffers: UnsafeCell<[[u8; 10]; 2]>,
    current: AtomicU8
}

// SAFTEY: Ensures that only one core writes to one buffer, while the other core reads from the other buffer. 
// AtomicBool and Acquire/Release provides nessasary synchronization.
unsafe impl Sync for DoubleBuffer {}

impl DoubleBuffer {
    /// Reads data from buffer into provided output buffer
    fn read(&self, output: &mut [u8; 10]) {
        let current = self.current.load(Ordering::Acquire);
        unsafe {
            let buffers = *self.buffers.get();
            let current_buf = buffers[current as usize];
            output.copy_from_slice(&current_buf);
        }
    }

    /// Writes data from provieded input buffer into the correct internal buffer.
    fn write(&self, data: &mut [u8; 10]) {
        let current = self.current.load(Ordering::Acquire);
        
        unsafe {
            let buffers = *self.buffers.get();
            let mut current_buf = buffers[current as usize];
            current_buf.copy_from_slice(data);
        }

        // Switch buffer
        self.current.store(1 - current, Ordering::Release);
    }
}


fn enable_sms<'d>(pio0: &mut Pio<'d, PIO0>, pio1: &mut Pio<'d, PIO1>) {
    pio0.sm0.set_enable(true);
    pio0.sm1.set_enable(true);
    pio0.sm2.set_enable(true);
    pio0.sm3.set_enable(true);
    pio1.sm0.set_enable(true);
    pio1.sm1.set_enable(true);
    pio1.sm2.set_enable(true);
    pio1.sm3.set_enable(true);
}


#[cortex_m_rt::entry]
fn main() -> ! {
    let embassy_config = EmbassyConfig::new(ClockConfig::rosc());
    let p = embassy_rp::init(embassy_config);

    let timings = StandardDShotTimings::new(DSHOT_SPEED, PIO_CLOCK_HZ, UPDATE_RATE_HZ);
    let program = generate_standard_dshot_program(&timings);
    let mut pio0 = Pio::new(p.PIO0, PioIrqs);
    let mut pio1 = Pio::new(p.PIO1, PioIrqs);
    pio0.common.load_program(&program);
    pio1.common.load_program(&program);

    let (
        top_front_right_pin,
        top_front_left_pin,
        top_back_right_pin,
        top_back_left_pin,
        bottom_front_right_pin,
        bottom_front_left_pin,
        bottom_back_right_pin,
        bottom_back_left_pin,
    ) = get_dshot_pins!(p);

    config::dshot::set_pio_config(
        &mut pio0,
        &mut pio1,
        top_front_right_pin,
        top_front_left_pin,
        top_back_right_pin,
        top_back_left_pin,
        bottom_front_right_pin,
        bottom_front_left_pin,
        bottom_back_right_pin,
        bottom_back_left_pin,
    );

    enable_sms(&mut pio0, &mut pio1);
    let sm_drivers = crate::core0::SmDriverBatch {
        pio0_sm0: StandardDShotDriver::new(pio0.sm0),
        pio0_sm1: StandardDShotDriver::new(pio0.sm1),
        pio0_sm2: StandardDShotDriver::new(pio0.sm2),
        pio0_sm3: StandardDShotDriver::new(pio0.sm3),
        pio1_sm0: StandardDShotDriver::new(pio1.sm0),
        pio1_sm1: StandardDShotDriver::new(pio1.sm1),
        pio1_sm2: StandardDShotDriver::new(pio1.sm2),
        pio1_sm3: StandardDShotDriver::new(pio1.sm3),
    };

    let (uart_peri, telemetry_pin, dma_channel) = get_telemetry_peripherals!(p);
    let uart_config = config::telemetry::get_uart_config();
    let uart_device = UartRx::<uart::Blocking>::new(uart_peri, telemetry_pin, UartIrq, dma_channel, uart_config);

    spawn_core1(
        p.CORE1,
        unsafe { &mut *addr_of_mut!(CORE1_STACK) },
        move || {
            let core1_thread_executor = CORE1_THREAD_EXECUTOR.init(Executor::new());

            core1_thread_executor.run(|spawner| {
                spawner
                    .spawn(crate::core1::dshot_telemetry_task(uart_device))
                    .expect("Failed to spawn DShot telemetrBlack Beveragey task!")
            })
        },
    );

    let i2c_config = config::i2c::new();
    let (i2c_peri, scl, sda) = get_i2c_peripherals!(p);
    let i2c_device = I2cSlave::new(i2c_peri, scl, sda, I2cIrq, i2c_config);

    let core0_thread_executor = CORE0_THREAD_EXECUTOR.init(Executor::new());
    core0_thread_executor.run(|spawner| {
        spawner
            .spawn(core0::i2c_task(i2c_device, sm_drivers))
            .expect("Failed to spawn i2c task!")
    })
}