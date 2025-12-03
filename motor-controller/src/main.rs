#![no_std]
#![no_main]

mod config;

use embassy_executor::{Executor, InterruptExecutor};
use embassy_rp::pio::{self, Pio};
use embassy_rp::{bind_interrupts, i2c_slave};
use embassy_rp::clocks::ClockConfig;
use embassy_rp::multicore::{spawn_core1, Stack};
use embassy_rp::peripherals::{I2C0, PIO0, PIO1};
use fixed::traits::ToFixed;
use rp2040_dshot::driver::StandardDShotDriver;
use rp2040_dshot::StandardDShotTimings;
use rp2040_dshot::encoder::Command as DShotCommand;
use rp2040_dshot::program::generate_standard_dshot_program;
use static_cell::StaticCell;
use embassy_rp::config::Config as EmbassyConfig;
use core::ptr::addr_of_mut;
use embassy_rp::interrupt::{self, InterruptExt, Priority};
use embassy_rp::i2c_slave::I2cSlave;
use embassy_rp::i2c;
use embassy_rp::pio::{InterruptHandler, Common};


use crate::config::dshot::{DSHOT_SPEED, PIO_CLOCK_HZ, UPDATE_RATE_HZ};

static mut CORE1_STACK: Stack<4096> = Stack::new();
static CORE0_THREAD_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_THREAD_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static CORE1_INTURRUPT_EXECUTOR: InterruptExecutor = InterruptExecutor::new();

bind_interrupts!(struct Irqs {
    I2C0_IRQ => i2c::InterruptHandler<I2C0>;
    PIO0_IRQ_0 => pio::InterruptHandler<PIO0>;
    PIO1_IRQ_0 => pio::InterruptHandler<PIO1>;
});

#[embassy_executor::task]
async fn dshot_telemetry_task() {

}

fn enable_sms<'d>(
    mut pio0: Pio<'d, PIO0>,
    mut pio1: Pio<'d, PIO1>
) {
    pio0.sm0.set_enable(true);
    pio0.sm1.set_enable(true);
    pio0.sm2.set_enable(true);
    pio0.sm3.set_enable(true);
    pio1.sm0.set_enable(true);
    pio1.sm1.set_enable(true);
    pio1.sm2.set_enable(true);
    pio1.sm3.set_enable(true);
}


#[embassy_executor::task]
async fn dshot_command_task(mut sms: SmBatch) {
    
}

#[embassy_executor::task]
async fn i2c_task(mut device: I2cSlave<'static, config::i2c::I2cPeripheral>, sms: SmBatch) {
    let mut buffer = [0u8; config::i2c::BUFFER_SIZE];
    loop {
        match device.listen(&mut buffer).await {
            Ok(i2c_slave::Command::Write(len)) => {
                let first_byte = buffer[0];
                match DShotCommand::try_from(first_byte) {
                    Ok(command) => {
                    }
                    Err(_) => todo!()
                }
            }
            Ok(_) => todo!(),
            Err(_) => todo!()
        }
    }
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

struct SmDriverBatch {
    pub pio0_sm0: StandardDShotDriver<'static, PIO0, 0>,
    pub pio0_sm1: StandardDShotDriver<'static, PIO0, 1>,
    pub pio0_sm2: StandardDShotDriver<'static, PIO0, 2>,
    pub pio0_sm3: StandardDShotDriver<'static, PIO0, 3>,
    pub pio1_sm0: StandardDShotDriver<'static, PIO1, 0>,
    pub pio1_sm1: StandardDShotDriver<'static, PIO1, 1>,
    pub pio1_sm2: StandardDShotDriver<'static, PIO1, 2>,
    pub pio1_sm3: StandardDShotDriver<'static, PIO1, 3>
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let embassy_config = EmbassyConfig::new(ClockConfig::rosc());
    let p = embassy_rp::init(embassy_config);

    let timings = StandardDShotTimings::new(DSHOT_SPEED, PIO_CLOCK_HZ, UPDATE_RATE_HZ);
    let program = generate_standard_dshot_program(&timings);
    let mut pio0 = Pio::new(p.PIO0, crate::Irqs);
    let mut pio1 = Pio::new(p.PIO1, crate::Irqs);
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

    enable_sms(pio0, pio1);
    let sm_drivers = SmDriverBatch{
        pio0_sm0: StandardDShotDriver::new(pio0.sm0),
        pio0_sm1: StandardDShotDriver::new(pio0.sm1),
        pio0_sm2: StandardDShotDriver::new(pio0.sm2),
        pio0_sm3: StandardDShotDriver::new(pio0.sm3),
        pio1_sm0: StandardDShotDriver::new(pio1.sm0),
        pio1_sm1: StandardDShotDriver::new(pio1.sm1),
        pio1_sm2: StandardDShotDriver::new(pio1.sm2),
        pio1_sm3: StandardDShotDriver::new(pio1.sm3),
    };

    spawn_core1(p.CORE1, unsafe { &mut *addr_of_mut!(CORE1_STACK) }, move || {
        let irq_dshot = interrupt::SWI_IRQ_0;
        irq_dshot.set_priority(Priority::P0);
        let spawner = CORE1_INTURRUPT_EXECUTOR.start(irq_dshot);

        spawner.spawn(dshot_command_task(sm_drivers)).expect("Failed to spawn DShot write task!");

        let core1_thread_executor = CORE1_THREAD_EXECUTOR.init(Executor::new());

        core1_thread_executor.run(|spawner| spawner.spawn(dshot_telemetry_task()).expect("Failed to spawn DShot telemetry task!"))
    });


    let i2c_config = config::i2c::new();
    let (i2c_peri, scl, sda) = get_i2c_peripherals!(p);
    let i2c_device = I2cSlave::new(i2c_peri, scl, sda, Irqs, i2c_config);

    let core0_thread_executor = CORE0_THREAD_EXECUTOR.init(Executor::new());
    core0_thread_executor.run(|spawner| spawner.spawn(i2c_task(i2c_device, sm_drivers)).expect("Failed to spawn i2c task!"))
}

