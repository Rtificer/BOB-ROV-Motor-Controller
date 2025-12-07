#![no_std]
#![no_main]

mod config;

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
use embassy_rp::{bind_interrupts, i2c_slave};
use embassy_time::TimeoutError;
use rp2040_dshot::StandardDShotTimings;
use rp2040_dshot::driver::{DShotDriver, StandardDShotDriver};
use rp2040_dshot::encoder::Command as DShotCommand;
use rp2040_dshot::program::generate_standard_dshot_program;
use static_cell::StaticCell;
use defmt::{warn, error, info};

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

// SAFTEY: Ensures that only one core writes to one buffer, while the other core reads from the other buffer. AtomicBool and Acquire/Release provides nessasary synchronization.
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


#[embassy_executor::task]
async fn dshot_telemetry_task(mut uart: UartRx<'static, uart::Blocking>) {
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


macro_rules! for_each_driver {
    ($batch: expr, |$driver: ident| $body:expr) => {{
        let $driver = &mut $batch.pio0_sm0;
        $body;
        let $driver = &mut $batch.pio0_sm1;
        $body;
        let $driver = &mut $batch.pio0_sm2;
        $body;
        let $driver = &mut $batch.pio0_sm3;
        $body;
        let $driver = &mut $batch.pio1_sm0;
        $body;
        let $driver = &mut $batch.pio1_sm1;
        $body;
        let $driver = &mut $batch.pio1_sm2;
        $body;
        let $driver = &mut $batch.pio1_sm3;
        $body;
    }};
}

async fn write_dshot(sms: &mut SmDriverBatch, buffer: &mut [u8; 2]) {
    let first_byte = buffer[0];

    if let Ok(command) = DShotCommand::try_from(first_byte) {
        // Handle as command
        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_command(command, true).await {
                // Match expression here to ensure all error varients are handled, (its 1, but ensure this throws an error if the error type is updated)
                match write_error {
                    TimeoutError => error!("Write Dshot command timeout error!"),
                }
                return;
            }
        })
    } else {
        // Handle as throttle
        let raw = u16::from_le_bytes([buffer[0], buffer[1]]);

        let Some(throttle) = raw.checked_sub(raw - 48) else {
            error!("Invalid raw value: {}", raw);
            return;
        };

        for_each_driver!(sms, |driver| {
            if let Err(write_error) = driver.write_throttle(throttle, true).await {
                match write_error {
                    rp2040_dshot::Error::ThrottleBoundsError { throttle } => {
                        error!("DShot throttle bounds error! throttle: {}", throttle)
                    }
                    rp2040_dshot::Error::TimeoutError(_) => {
                        error!("Write DShot throttle timeout error!")
                    }
                    _ => error!("Unknown write DShot throttle error!"),
                }
                return;
            }
        })
    }
}

fn handle_respond_to_read_result(result: Result<i2c_slave::ReadStatus, i2c_slave::Error>) {
    match result {
        Ok(i2c_slave::ReadStatus::Done) => (),
        Ok(i2c_slave::ReadStatus::NeedMoreBytes) => warn!("i2c telemetry read failed: Controller attempted to read more bytes than provided!"),
        Ok(i2c_slave::ReadStatus::LeftoverBytes(count)) => warn!("i2c telemetry read failed: Controller stopped reading with {} leftover bytes!", count),
        Err(error) => handle_i2c_error(error),
    }
}

fn handle_i2c_error(error: i2c_slave::Error) {
    match error {
        i2c_slave::Error::Abort(reason) => {
            match reason {
                i2c::AbortReason::NoAcknowledge => error!("I2C Aborted! Bus operation not acknowledged!"),
                i2c::AbortReason::ArbitrationLoss => error!("I2C Aborted! Abritration lost!"),
                i2c::AbortReason::TxNotEmpty(remaining) => error!("I2C Aborted! Transmit ended with data still in fifo! Remaining: {}", remaining),
                i2c::AbortReason::Other(data) => error!("I2C aborted! Reason unknown! Attatched Data (idk what this is): {}", data)
            }
        }
        i2c_slave::Error::InvalidResponseBufferLength => error!("I2C read buffer is 0 length!"),
        _ => error!("Unknown i2c_slave error!")
    }
}

#[embassy_executor::task]
async fn i2c_task(
    mut device: I2cSlave<'static, config::i2c::I2cPeripheral>,
    mut sms: SmDriverBatch,
) {
    let mut write_buffer = [0u8; 2];
    let mut telemetry_buffer = [0u8; 10];

    loop {
        match device.listen(&mut write_buffer).await {
            Ok(i2c_slave::Command::Write(_)) => {
                write_dshot(&mut sms, &mut write_buffer).await;
            }
            Ok(i2c_slave::Command::Read) => {
                TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
                handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
            }
            Ok(i2c_slave::Command::WriteRead(_)) => {
                write_dshot(&mut sms, &mut write_buffer).await;
                
                TELEMETRY_BUFFERS.read(&mut telemetry_buffer);
                handle_respond_to_read_result(device.respond_to_read(&telemetry_buffer).await);
            },
            Ok(i2c_slave::Command::GeneralCall(_)) => warn!("Recieved erronious GeneralCall i2c instruction!"),
            Err(error) => handle_i2c_error(error),
        }
    }
}

struct SmDriverBatch {
    pub pio0_sm0: StandardDShotDriver<'static, PIO0, 0>,
    pub pio0_sm1: StandardDShotDriver<'static, PIO0, 1>,
    pub pio0_sm2: StandardDShotDriver<'static, PIO0, 2>,
    pub pio0_sm3: StandardDShotDriver<'static, PIO0, 3>,
    pub pio1_sm0: StandardDShotDriver<'static, PIO1, 0>,
    pub pio1_sm1: StandardDShotDriver<'static, PIO1, 1>,
    pub pio1_sm2: StandardDShotDriver<'static, PIO1, 2>,
    pub pio1_sm3: StandardDShotDriver<'static, PIO1, 3>,
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
    let sm_drivers = SmDriverBatch {
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
                    .spawn(dshot_telemetry_task(uart_device))
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
            .spawn(i2c_task(i2c_device, sm_drivers))
            .expect("Failed to spawn i2c task!")
    })
}