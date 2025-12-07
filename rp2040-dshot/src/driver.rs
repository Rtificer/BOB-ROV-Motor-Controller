use crate::encoder::{
    DShotVariant, ERpmVarient, Frame, InvertedDShotVariant, StandardDShotVariant, Command
};
use core::marker::PhantomData;
use core::ptr;
use embassy_executor::Spawner;
use embassy_rp::peripherals::{PIO0, PIO1};
use embassy_rp::pio::{StateMachineRx, StateMachineTx};
use embassy_rp::pio::{Instance, Irq, StateMachine};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::channel::Channel;
use embassy_time::{Duration, TimeoutError, with_timeout};
use static_cell::StaticCell;


trait PrivateDShotDriver<
    'd,
    PIO: Instance + 'd, 
    const SM: usize
>: Sized {
    type Variant: DShotVariant;

    /// Returns a mutable reference to the Driver's StateMachineTx
    fn tx(&mut self) -> &mut StateMachineTx<'d, PIO, SM>;

    /// Waits for FIFO TX to be ready or until 500us have elapsed
    #[allow(async_fn_in_trait)]
    async fn write_frame(&mut self, frame: Frame<Self::Variant>) -> Result<(), TimeoutError> {
        with_timeout(Duration::from_micros(500), self.tx().wait_push(frame.inner() as u32)).await
    }

    /// Attempts to push frame data to the TX FIFO 
    fn try_write_frame(&mut self, frame: Frame<Self::Variant>) -> bool {
        self.tx().try_push(frame.inner() as u32)
    }
}

#[allow(private_bounds)]
pub trait DShotDriver<
    'd, 
    PIO: Instance + 'd, 
    const SM: usize
>: PrivateDShotDriver<'d, PIO, SM>
{
    #[allow(async_fn_in_trait)]
    async fn write_throttle(
        &mut self,
        throttle: u16,
        request_telemetry: bool,
    ) -> Result<(), crate::Error> {
        let frame = Frame::<Self::Variant>::from_throttle(throttle, request_telemetry)
            .ok_or(crate::Error::ThrottleBoundsError { throttle })?;

        Ok(PrivateDShotDriver::write_frame(self, frame).await?)
    }

    fn try_write_throttle(
        &mut self,
        throttle: u16,
        request_telemetry: bool,
    ) -> Result<(), crate::Error> {
        let frame = Frame::<Self::Variant>::from_throttle(throttle, request_telemetry)
            .ok_or(crate::Error::ThrottleBoundsError { throttle })?;

        self.try_write_frame(frame)
            .then_some(())
            .ok_or(crate::Error::TxTryPushFaliure)
    }

    #[allow(async_fn_in_trait)]
    async fn write_command(&mut self, command: Command, request_telemetry: bool) -> Result<(), TimeoutError>{
        let frame = Frame::<Self::Variant>::from_command(command, request_telemetry);

        Ok(self.write_frame(frame).await?)
    }

    fn try_write_command(&mut self, command: Command, request_telemetry: bool) -> bool {
        let frame = Frame::<Self::Variant>::from_command(command, request_telemetry);

        self.try_write_frame(frame)
    }
}


pub struct StandardDShotDriver<
    'd,
    PIO: Instance,
    const SM: usize,
> {
    sm: StateMachine<'d, PIO, SM>,
    _protocol: PhantomData<StandardDShotVariant>,
}

impl<'d, PIO: Instance, const SM: usize> 
    PrivateDShotDriver<'d, PIO, SM> 
    for StandardDShotDriver<'d, PIO, SM>
{
    type Variant = StandardDShotVariant; 

    fn tx(&mut self) -> &mut StateMachineTx<'d, PIO, SM> {
        self.sm.tx()
    }
}

impl<'d, PIO: Instance, const SM: usize>
    DShotDriver<'d, PIO, SM>
    for StandardDShotDriver<'d, PIO, SM>
{}

impl<'d, PIO: Instance, const SM: usize>
    StandardDShotDriver<'d, PIO, SM>
{
    pub fn new(
        sm: StateMachine<'d, PIO, SM>,
    ) -> Self {
        // Self::setup_config(common, &mut sm, pin, dshot_speed)?;
        Self {
            sm,
            _protocol: PhantomData,
        }
    }
}

pub struct BdDShotDriver<
    PIO: Instance + 'static,
    const SM: usize
> {
    tx_ref: &'static mut StateMachineTx<'static, PIO, SM>,
    channel: &'static Channel<NoopRawMutex, u16, 3>,
    _protocol: PhantomData<InvertedDShotVariant>,
}

impl<PIO: Instance, const SM: usize>
    PrivateDShotDriver<'static, PIO, SM>
    for BdDShotDriver<PIO, SM>
{
    type Variant = StandardDShotVariant;

    fn tx(&mut self) -> &mut StateMachineTx<'static, PIO, SM> {
        self.tx_ref
    }
}


impl<PIO: Instance, const SM: usize>
    DShotDriver<'static, PIO, SM>
    for BdDShotDriver<PIO, SM>
{}

impl<PIO: Instance, const SM: usize>
    BdDShotDriver<PIO, SM>
{
    /// Reads the next telemetry value from the channel 
    /// 
    /// If there are no messages in the channel, this method will wait until 500us have passed before returning [`crate::Error::TimeoutError`].
    /// 
    /// Returns [`crate::Error::InvalidTelemetryChecksum`] if the crc checksum from read telemetry packet is invalid.
    pub async fn read_telemetry<V: ERpmVarient>(&self) -> Result<V, crate::Error> {
        let raw = with_timeout(Duration::from_micros(500), self.channel.receive()).await?;
        V::from_raw(raw).ok_or(crate::Error::InvalidTelemetryChecksum)
    }

    /// Reads the next telemetry value from the channel 
    /// 
    /// Returns [`crate::Error::TryReceiveError`] if the channel is empty.
    /// 
    /// Returns [`crate::Error::InvalidTelemetryChecksum`] if the crc checksum from read telemetry packet is invalid.
    pub fn try_read_telemerty<V: ERpmVarient>(&self) -> Result<V, crate::Error> {
        let raw = self.channel.try_receive()?;
        V::from_raw(raw).ok_or(crate::Error::InvalidTelemetryChecksum)
    }
}

/// Macro to generate BdDshotDriver::new() for each PIO and SM
macro_rules! impl_bd_dshot_driver {
    ($pio:ty, $sm:expr) => {
        pastey::paste! {

            // Create MaybeUnits to store static lifetime StateMachines.
            static [<RX_STORAGE_ $pio:upper _SM $sm>]: StaticCell<StateMachineRx<'static, $pio, $sm>> = StaticCell::new();
            static [<TX_STORAGE_ $pio:upper _SM $sm>]: StaticCell<StateMachineTx<'static, $pio, $sm>> = StaticCell::new();

            impl BdDShotDriver<$pio, $sm>
            {
                #[doc = "Creates a new BdDshot Driver instance for " $pio " and SM " $sm "."]
                #[doc = ""]
                #[doc = "If config generation fails from clock division conversion returns [`crate::Error::ClockDividerConversionError`]"]
                pub fn new(
                    mut sm: StateMachine<'static, $pio, $sm>,
                    irq: Irq<'static, $pio, $sm>,
                    channel: &'static Channel<NoopRawMutex, u16, 3>,
                    spawner: &Spawner
                ) -> Result<Self, crate::Error> {
                    // Get rx and tf refrences in current (non-static) lifetime
                    let (rx_value, tx_value) = sm.rx_tx();
                    
                    let rx_ref = [<RX_STORAGE_ $pio:upper _SM $sm>].init(unsafe { ptr::read(rx_value)});
                    let tx_ref = [<TX_STORAGE_ $pio:upper _SM $sm>].init(unsafe { ptr::read(tx_value)});

                    // Spawn the erpm_reader task (for the given PIO and SM)
                    // Nessasary because embassy tasks can take generic arguments
                    spawner.spawn([<erpm_reader_task_ $pio:lower _sm $sm>](irq, rx_ref, channel))?;

                    Ok(Self{
                        tx_ref,
                        channel,
                        _protocol: PhantomData
                    })
                }
            }
        }
    }
}


impl_bd_dshot_driver!(PIO0, 0);
impl_bd_dshot_driver!(PIO0, 1);
impl_bd_dshot_driver!(PIO0, 2);
impl_bd_dshot_driver!(PIO0, 3);
impl_bd_dshot_driver!(PIO1, 0);
impl_bd_dshot_driver!(PIO1, 1);
impl_bd_dshot_driver!(PIO1, 2);
impl_bd_dshot_driver!(PIO1, 3);

macro_rules! generate_erpm_reader {
    ($pio:ty, $sm:expr) => {
        pastey::paste! {
            #[embassy_executor::task]
            async fn [<erpm_reader_task_ $pio:lower _sm $sm>](
                irq: Irq<'static, $pio, $sm>,
                rx_ref: &'static mut StateMachineRx<'static, $pio, $sm>,
                channel: &'static Channel<NoopRawMutex, u16, 3>
            ) {
                erpm_reader_task_impl(irq, rx_ref, channel).await;
            }
        }
    };
}

generate_erpm_reader!(PIO0, 0);
generate_erpm_reader!(PIO0, 1);
generate_erpm_reader!(PIO0, 2);
generate_erpm_reader!(PIO0, 3);
generate_erpm_reader!(PIO1, 0);
generate_erpm_reader!(PIO1, 1);
generate_erpm_reader!(PIO1, 2);
generate_erpm_reader!(PIO1, 3);

const GCR_DECODING_MAP: [Option<u8>; 32] = [
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    None,
    Some(0b_1001), // 0b_01001 -> 0b_1001
    Some(0b_1010), // 0b_01010 -> 0b_1010
    Some(0b_1011), // 0b_01011 -> 0b_1011
    None,
    Some(0b_1101), // 0b_01101 -> 0b_1101
    Some(0b_1110), // 0b_01110 -> 0b_1110
    Some(0b_1111), // 0b_01111 -> 0b_1111
    None,
    None,
    Some(0b_0010), // 0b_10010 -> 0b_0010
    Some(0b_0011), // 0b_10011 -> 0b_0011
    None,
    Some(0b_0101), // 0b_10101 -> 0b_0101
    Some(0b_0110), // 0b_10110 -> 0b_0110
    Some(0b_0111), // 0b_10111 -> 0b_0111
    None,
    Some(0b_0000), // 0b_11001 -> 0b_0000
    Some(0b_1000), // 0b_11010 -> 0b_1000
    Some(0b_0001), // 0b_11011 -> 0b_0001
    None,
    Some(0b_0100), // 0b_11101 -> 0b_0100
    Some(0b_1100), // 0b_11110 -> 0b_1100
    None,
];

fn decode_gcr(gcr: u32) -> Option<u16> {
    let mut result: u16 = 0;
    for shift in 1..=4 {
        let index = ((gcr >> (shift * 5)) & 0x1F) as usize;
        let nibble = GCR_DECODING_MAP[index]?;
        result |= (nibble as u16) << (shift * 4)
    }
    Some(result)
}

async fn erpm_reader_task_impl<'d, PIO: Instance, const SM: usize>(
    mut irq: Irq<'static, PIO, SM>,
    rx_ref: &'static mut StateMachineRx<'d, PIO, SM>,
    channel: &'static Channel<NoopRawMutex, u16, 3>,
) {
    loop {
        if with_timeout(Duration::from_micros(500), irq.wait())
            .await
            .is_err()
        {
            defmt::error!("Failed to read erpm data from PIO {}: irq flag timeout", SM);
            continue;
        }
        

        let Some(value) = rx_ref.try_pull() else {
            defmt::error!("Failed to read erpm data from PIO {}: rx pull failed", SM);
            continue;
        };

        let gcr = value ^ (value >> 1);

        let Some(data) = decode_gcr(gcr) else {
            defmt::error!("Failed to read erpm data from PIO {}: gcr decode failed", SM);
            continue;
        };

        if with_timeout(Duration::from_micros(500), channel.send(data))
            .await
            .is_err()
        {
            if channel.is_full() {
                defmt::warn!("Failed to read erpm data from PIO {}: send channel is full! Is the chip overloaded?", SM)
            } else {
                defmt::warn!("Failed to read erpm data from PIO {}: unknown data send timeout", SM)
            }
        }
    }
}
