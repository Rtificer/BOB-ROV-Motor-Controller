use core::{marker::PhantomData, num::NonZeroU32};
use num_enum::TryFromPrimitive;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DShotSpeed {
    DShot150,
    DShot300,
    DShot600,
    DShot1200,
}

impl DShotSpeed {
    /// Returns the [`f32`] bit time in microseconds, corresponding to the given enum varient.
    #[must_use]
    pub const fn bit_time_us(&self) -> f32 {
        match self {
            Self::DShot150 => 6.666_667,
            Self::DShot300 => 3.333_333,
            Self::DShot600 => 1.666_667,
            Self::DShot1200 => 0.833_333,
        }
    }

    /// Returns the [`u32`] bit rate in hertz, corresponding to the given enum varient.
    #[must_use]
    pub const fn bit_rate_hz(&self) -> u32 {
        match self {
            Self::DShot150 => 150_000,
            Self::DShot300 => 300_000,
            Self::DShot600 => 600_000,
            Self::DShot1200 => 1_200_000,
        }
    }

    /// Returns the [`u32`] bit rate in hertz, when using GCR encoding (during Erpm transmission in BD-Dshot), corresponding to the given enum varient.
    #[must_use]
    pub const fn gcr_bit_rate_hz(&self) -> u32 {
        match self {
            Self::DShot150 => 187_500,
            Self::DShot300 => 375_000,
            Self::DShot600 => 750_000,
            Self::DShot1200 => 1_500_000,
        }
    }
}

pub trait DShotVariant {
    /// Creates a new [`DShotVariant`] given a [`DShotSpeed`] value.
    #[must_use]
    fn new(speed: DShotSpeed) -> Self;

    /// Computes the crc value from raw [`u16`] frame data
    #[must_use]
    fn compute_crc(value: u16) -> u8;

    /// Returns the inner speed value.
    #[must_use]
    fn inner(&self) -> DShotSpeed;

    /// Const for checking if the ``DShot`` protocol is inverted
    const IS_INVERTED: bool;
}

// Const helper functions for CRC computation (update when const trait functions becomes stable)
#[must_use]
const fn compute_standard_crc(value: u16) -> u8 {
    ((value ^ (value >> 4) ^ (value >> 8)) & 0x0F) as u8
}
#[must_use]
const fn compute_inverted_crc(value: u16) -> u8 {
    ((!(value ^ (value >> 4) ^ (value >> 8))) & 0x0F) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardDShotVariant {
    inner: DShotSpeed,
}

impl DShotVariant for StandardDShotVariant {
    fn new(speed: DShotSpeed) -> Self {
        Self { inner: speed }
    }

    fn compute_crc(value: u16) -> u8 {
        compute_standard_crc(value)
    }

    fn inner(&self) -> DShotSpeed {
        self.inner
    }

    const IS_INVERTED: bool = false;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvertedDShotVariant {
    inner: DShotSpeed,
}

impl DShotVariant for InvertedDShotVariant {
    fn new(speed: DShotSpeed) -> Self {
        Self { inner: speed }
    }

    fn compute_crc(value: u16) -> u8 {
        compute_inverted_crc(value)
    }

    fn inner(&self) -> DShotSpeed {
        self.inner
    }

    const IS_INVERTED: bool = true;
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Frame<P: DShotVariant> {
    inner: u16,
    _protocol: PhantomData<P>,
}

pub type StandardFrame = Frame<StandardDShotVariant>;
pub type InvertedFrame = Frame<InvertedDShotVariant>;

impl<P: DShotVariant> Frame<P> {
    /// Creates a new option of a frame given a speed (0-1999) and a telemetry toggle
    ///
    /// Returns [`None`] if the speed is out of bounds
    #[must_use]
    pub const fn from_throttle(throttle: u16, request_telemetry: bool) -> Option<Self> {
        if throttle >= 2000 {
            return None;
        }

        Some(Self::construct_frame(throttle + 48, request_telemetry))
    }

    /// Creates a new frame given a [`Command`] and a telemetry toggle
    #[must_use]
    pub const fn from_command(command: Command, request_telemetry: bool) -> Self {
        Self::construct_frame(command as u16, request_telemetry)
    }

    #[must_use]
    const fn construct_frame(data: u16, request_telemetry: bool) -> Self {
        let mut data = data << 5;

        if request_telemetry {
            data |= 0x10;
        }

        data = (data & !0x0F) | Self::compute_crc(data) as u16;

        Self {
            inner: data,
            _protocol: PhantomData,
        }
    }

    #[must_use]
    const fn compute_crc(data: u16) -> u8 {
        if P::IS_INVERTED {
            compute_inverted_crc(data)
        } else {
            compute_standard_crc(data)
        }
    }

    /// Returns a option of the speed value (0-1999).
    ///
    /// Returns [`None`] if inner value is a command.
    #[must_use]
    pub const fn speed(&self) -> Option<u16> {
        (self.inner >> 5).checked_sub(48)
    }

    /// Returns the status of the telemetry toggle
    #[must_use]
    pub const fn telemetry_enabled(&self) -> bool {
        (self.inner & 0x10) != 0
    }

    /// Returns the CRC checksum
    #[must_use]
    pub const fn crc(&self) -> u16 {
        self.inner & 0x0F
    }

    /// Returns the raw [`u16`] data
    #[must_use]
    pub const fn inner(&self) -> u16 {
        self.inner
    }
}

/// Commands that occupy the lower 48 speed values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Command {
    MotorStop,
    /// Wait at least 260ms before next command.
    Beep1,
    /// Wait at least 260ms before next command.
    Beep2,
    /// Wait at least 260ms before next command.
    Beep3,
    /// Wait at least 260ms before next command.
    Beep4,
    /// Wait at least 260ms before next command.
    Beep5,
    /// Wait at least 12ms before next command.
    ESCInfo,
    /// Needs 6 transmissions.
    SpinDirection1,
    /// Needs 6 transmissions.
    SpinDirection2,
    /// Needs 6 transmissions.
    ThreeDModeOn,
    /// Needs 6 transmissions.
    ThreeDModeOff,
    SettingsRequest,
    /// Needs 6 transmissions. Wait at least 35ms before next command.
    SettingsSave,
    /// Needs 6 transmissions.
    ExtendedTelemetryEnable,
    /// Needs 6 transmissions.
    ExtendedTelemetryDisable,

    // 15-19 are unassigned.
    /// Needs 6 transmissions.
    SpinDirectionNormal = 20,
    /// Needs 6 transmissions.
    SpinDirectonReversed,
    Led0On,
    Led1On,
    Led2On,
    Led3On,
    Led0Off,
    Led1Off,
    Led2Off,
    Led3Off,
    AudioStreamModeToggle,
    SilentModeToggle,
    /// Needs 6 transmissions. Enables individual signal line commands.
    SignalLineTelemetryEnable,
    /// Needs 6 transmissions. Disables individual signal line commands.
    SignalLineTelemetryDisable,
    /// Needs 6 transmissions. Enables individual signal line commands.
    SignalLineContinuousERPMTelemetry,
    /// Needs 6 transmissions. Enables individual signal line commands.
    SignalLineContinuousERPMPeriodTelemetry,

    // 36-41 are unassigned.
    /// 1ºC per LSB.
    SignalLineTemperatureTelemetry = 42,
    /// 10mV per LSB, 40.95V max.
    SignalLineVoltageTelemetry,
    /// 100mA per LSB, 409.5A max.
    SignalLineCurrentTelemetry,
    /// 10mAh per LSB, 40.95Ah max.
    SignalLineConsumptionTelemetry,
    /// 100erpm per LSB, 409500erpm max.
    SignalLineERPMTelemetry,
    /// 16us per LSB, 65520us max.
    SignalLineERPMPeriodTelemetry,
}

// Gets the period shift value from raw frame data
#[must_use]
const fn shift_from_raw(raw: u16) -> u8 {
    ((raw >> 12) & 0x07) as u8
}

// Gets period base value from raw frame data
#[must_use]
const fn base_from_raw(raw: u16) -> u16 {
    (raw >> 3) & 0x01FF
}

pub trait ERpmVarient: Sized {
    /// Creates a new option of a ERPM frame object given the raw frame data (after grc decoding)
    ///
    /// Returns [`None`] if crc checksum is invalid.
    #[must_use]
    fn from_raw(raw: u16) -> Option<Self>;

    /// Computes the CRC checksum from raw frame data
    #[must_use]
    fn compute_crc(raw: u16) -> u8 {
        // crc for erpm is computed without inversion
        compute_standard_crc(raw)
    }

    #[must_use]
    fn crc(&self) -> u8;

    /// Gets the CRC checksum from from raw frame data
    #[must_use]
    fn crc_from_raw(raw: u16) -> u8 {
        (raw & 0x0F) as u8
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct StandardERpmFrame {
    shift: u8,
    base: u16,
    crc: u8,
}

impl ERpmVarient for StandardERpmFrame {
    fn from_raw(raw: u16) -> Option<Self> {
        let crc_raw = Self::crc_from_raw(raw);
        let crc_computed = Self::compute_crc(raw);

        (crc_raw == crc_computed).then_some(Self {
            shift: shift_from_raw(raw),
            base: base_from_raw(raw),
            crc: crc_raw,
        })
    }

    fn crc(&self) -> u8 {
        self.crc
    }
}

impl StandardERpmFrame {
    /// Computes [`NonZeroU32`] motor period in us.
    ///
    /// Returns [`None`] when the motor is stopped (either base or shift is 0)
    #[must_use]
    pub fn compute_period_us(&self) -> Option<NonZeroU32> {
        if self.base == 0 {
            return None;
        }

        let period = (u32::from(self.base)) << self.shift;
        NonZeroU32::new(period)
    }

    /// Computes [`u32`] motor RPM.
    #[must_use]
    pub fn compute_rpm(&self) -> u32 {
        self.compute_period_us()
            .map_or(0, |period| 60_000_000 / period)
    }

    /// Returns internal 3 bit [`u8`] ``period_us`` shift value.
    #[must_use]
    pub fn shift(&self) -> u8 {
        self.shift
    }

    /// Returns internal 9 bit [`u16`] ``period_us`` base value.
    #[must_use]
    pub fn base(&self) -> u16 {
        self.base
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtendedERpmData {
    /// Speed in Rpm
    Rpm {
        shift: u8,
        base: u16,
    },
    /// Temperature in degrees c (0-255)
    Temperature(u8),
    /// Voltage 0.25V per step
    Voltage(u8),
    /// Current in amps
    Current(u8),
    Debug1(u8),
    Debug2(u8),
    Debug3(u8),
    StateOrEvent(u8),
}

impl ExtendedERpmData {
    const fn from_raw(raw: u16) -> Self {
        let data = ((raw >> 4) & 0xFF) as u8;
        match raw >> 12 {
            0x02 => ExtendedERpmData::Temperature(data),
            0x04 => ExtendedERpmData::Voltage(data),
            0x06 => ExtendedERpmData::Current(data),
            0x08 => ExtendedERpmData::Debug1(data),
            0x0A => ExtendedERpmData::Debug2(data),
            0x0C => ExtendedERpmData::Debug3(data),
            0x0E => ExtendedERpmData::StateOrEvent(data),
            _ => ExtendedERpmData::Rpm {
                shift: shift_from_raw(raw),
                base: base_from_raw(raw),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtendedERpmFrame {
    data: ExtendedERpmData,
    crc: u8,
}

impl ERpmVarient for ExtendedERpmFrame {
    fn from_raw(raw: u16) -> Option<Self> {
        let crc_raw = Self::crc_from_raw(raw);
        let crc_computed = Self::compute_crc(raw);

        if crc_raw != crc_computed {
            return None;
        }

        Some(Self {
            data: ExtendedERpmData::from_raw(raw),
            crc: crc_raw,
        })
    }

    fn crc(&self) -> u8 {
        self.crc
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeriodComputationResult {
    /// Indicates that the packet is not an Rpm Packet
    NotRpmPacket,
    /// Indicates that the motor has been stopped (either base or shift is 0)
    StoppedMotor,
}

impl ExtendedERpmFrame {
    /// Computes [`NonZeroU32`] motor period in us.
    ///
    /// # Errors
    /// 
    /// Returns [`PeriodComputationResult::StoppedMotor`] when the motor is stopped (either base or shift is 0)
    /// Returns [`PeriodComputationResult::NotRpmPacket`] when the packet type is not RPM.
    pub fn compute_period_us(&self) -> Result<NonZeroU32, PeriodComputationResult> {
        let ExtendedERpmData::Rpm { shift, base } = self.data else {
            return Err(PeriodComputationResult::NotRpmPacket);
        };

        if base == 0 {
            return Err(PeriodComputationResult::StoppedMotor);
        }

        let period = (u32::from(base)) << shift;
        NonZeroU32::new(period).ok_or(PeriodComputationResult::StoppedMotor)
    }

    /// Computes [`u32`] motor Rpm.
    ///
    /// # Errors
    /// 
    /// Returns [`PeriodComputationResult::NotRpmPacket`] when the packet type is not RPM.
    pub fn compute_rpm(&self) -> Result<u32, PeriodComputationResult> {
        match self.compute_period_us() {
            Ok(period) => Ok(60_000_000 / period),
            Err(PeriodComputationResult::StoppedMotor) => Ok(0),
            Err(e) => Err(e),
        }
    }

    /// Returns internal 3 bit [`u8`] ``period_us`` shift value.
    ///
    /// Returns [`None`] when the packet is not of type Rpm.
    #[must_use]
    pub fn shift(&self) -> Option<u8> {
        match self.data {
            ExtendedERpmData::Rpm { shift, .. } => Some(shift),
            _ => None,
        }
    }

    /// Returns internal 9 bit [`u16`] ``period_us`` base value.
    ///
    /// Returns [`None`] when the packet is not of type Rpm.
    #[must_use]
    pub fn base(&self) -> Option<u16> {
        match self.data {
            ExtendedERpmData::Rpm { base, .. } => Some(base),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "driver", derive(defmt::Format))]
pub struct TelemetryFrame {
    /// deg C
    temp: u8,
    /// centivolts
    voltage: u16,
    /// centiamps
    current: u16,
    /// mAh
    consumption: u16,
    /// ERPM / 100 (to get real rpm mutliply by 2 / (magnetpol count))
    e_rpm: u16,
    /// crc checksum
    crc: u8
}

impl TelemetryFrame {
    /// Creates an new option of a [`TelemetryFrame`] instance from raw 80 byte data.
    /// 
    /// Returns [`None`] when crc checksum is invalid
    #[must_use]
    pub fn from_bytes(data: &[u8; 10]) -> Option<Self> {
        let crc = Self::compute_crc(&data[..9]);
        if crc != data[9] {
            return None;
        }

        Some(Self {
            temp: data[0],
            voltage: u16::from_le_bytes([data[1], data[2]]),
            current: u16::from_le_bytes([data[3], data[4]]),
            consumption: u16::from_le_bytes([data[5], data[6]]),
            e_rpm: u16::from_le_bytes([data[7], data[8]]),
            crc: data[9],
        })
    }

    /// Computes the [`u8`] crc checksum from telemetry byte data
    #[must_use]
    pub fn compute_crc(data: &[u8]) -> u8 {
        let mut crc: u8 = 0;
        for &byte in data {
            crc ^= byte;
            for _ in 0..8 {
                if crc & 0x80 != 0 {
                    crc = (crc << 1) ^ 0x07;
                } else {
                    crc <<= 1;
                }
            }
        }
        crc
    }
}
