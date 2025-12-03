use core::{marker::PhantomData, num::NonZeroU32};

use num_enum::{FromPrimitive, TryFromPrimitive};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DShotSpeed {
    DShot150,
    DShot300,
    DShot600,
    DShot1200,
}

impl DShotSpeed {
    /// Returns the [`f32`] bit time in microseconds, corresponding to the given enum varient.
    pub const fn bit_time_us(&self) -> f32 {
        match self {
            Self::DShot150 => 6.666667,
            Self::DShot300 => 3.333333,
            Self::DShot600 => 1.666667,
            Self::DShot1200 => 0.833333,
        }
    }

    /// Returns the [`u32`] bit rate in hertz, corresponding to the given enum varient.
    pub const fn bit_rate_hz(&self) -> u32 {
        match self {
            Self::DShot150 => 150_000,
            Self::DShot300 => 300_000,
            Self::DShot600 => 600_000,
            Self::DShot1200 => 1_200_000,
        }
    }

    /// Returns the [`u32`] bit rate in hertz, when using GCR encoding (during Erpm transmission in BD-Dshot), corresponding to the given enum varient.
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
    /// Creates a new DShotVarient given a [`DShotSpeed`] value.
    fn new(speed: DShotSpeed) -> Self;

    /// Computes the crc value from raw [`u16`] frame data
    fn compute_crc(value: u16) -> u8;
    
    /// Returns the inner speed value.
    fn inner(&self) -> DShotSpeed;

    /// Const for checking if the DShot protocol is inverted
    const IS_INVERTED: bool;
}

// Const helper functions for CRC computation (update when const trait functions becomes stable)
const fn compute_standard_crc(value: u16) -> u8 {
    ((value ^ (value >> 4) ^ (value >> 8)) & 0x0F) as u8
}

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
    pub const fn from_throttle(throttle: u16, request_telemetry: bool) -> Option<Self> {
        if throttle >= 2000 {
            return None;
        }

        Some(Self::construct_frame(throttle + 48, request_telemetry))
    }

    /// Creates a new frame given a [`Command`] and a telemetry toggle
    pub const fn from_command(command: Command, request_telemetry: bool) -> Self {
        Self::construct_frame(command as u16, request_telemetry)
    }

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
    pub const fn speed(&self) -> Option<u16> {
        (self.inner >> 5).checked_sub(48)
    }

    /// Returns the status of the telemetry toggle.
    pub const fn telemetry_enabled(&self) -> bool {
        (self.inner & 0x10) != 0
    }

    /// Returns the CRC checksum
    pub const fn crc(&self) -> u16 {
        self.inner & 0x0F
    }

    /// Returns the raw [`u16`] data
    pub const fn inner(&self) -> u16 {
        self.inner
    }
}

/// Commands that occupy the lower 48 speed values.
#[derive(Copy, Clone, Debug, PartialEq, Eq, TryFromPrimitive)]
#[repr(u8)]
pub enum Command {
    MotorStop
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
const fn shift_from_raw(raw: u16) -> u8 {
    ((raw >> 12) & 0x07) as u8
}

// Gets period base value from raw frame data
const fn base_from_raw(raw: u16) -> u16 {
    (raw >> 3) & 0x01FF
}

pub trait ERpmVarient: Sized {
    /// Creates a new option of a ERpm frame object given the raw frame data (after grc decoding)
    ///
    /// Returns [`None`] if crc checksum is invalid.
    fn from_raw(raw: u16) -> Option<Self>;

    /// Computes the CRC checksum from raw frame data
    fn compute_crc(raw: u16) -> u8 {
        // crc for erpm is computed without inversion
        compute_standard_crc(raw)
    }

    fn crc(&self) -> u8;

    /// Gets the CRC checksum from from raw frame data
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
        let crc_computer = Self::compute_crc(raw);

        (crc_raw == crc_computer).then_some(Self {
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
    pub fn compute_period_us(&self) -> Option<NonZeroU32> {
        if self.base == 0 {
            return None;
        }

        let period = (self.base as u32) << self.shift;
        NonZeroU32::new(period)
    }

    /// Computes [`u32`] motor Rpm.
    pub fn compute_rpm(&self) -> u32 {
        self.compute_period_us()
            .map(|period| 60_000_000 / period)
            .unwrap_or(0)
    }

    /// Returns internal 3 bit [`u8`] period_us shift value.
    pub fn shift(&self) -> u8 {
        self.shift
    }

    /// Returns internal 9 bit [`u16`] period_us base value.
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
    const fn from_raw(raw: u16) -> Option<Self> {
        let data = ((raw >> 4) & 0xFF) as u8;
        match raw >> 12 {
            0x02 => Some(ExtendedERpmData::Temperature(data)),
            0x04 => Some(ExtendedERpmData::Voltage(data)),
            0x06 => Some(ExtendedERpmData::Current(data)),
            0x08 => Some(ExtendedERpmData::Debug1(data)),
            0x0A => Some(ExtendedERpmData::Debug2(data)),
            0x0C => Some(ExtendedERpmData::Debug3(data)),
            0x0E => Some(ExtendedERpmData::StateOrEvent(data)),
            _ => {
                Some(ExtendedERpmData::Rpm { 
                    shift: shift_from_raw(raw),
                    base: base_from_raw(raw),
                })
            } 
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

        Some(Self{ data: ExtendedERpmData::from_raw(raw)?, crc: crc_raw })
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
    /// Returns [`PeriodComputationResult::StoppedMotor`] when the motor is stopped (either base or shift is 0)
    /// 
    /// Returns [`PeriodComputationResult::NotRpmPacket`] when the packet type is not RPM.
    pub fn compute_period_us(&self) -> Result<NonZeroU32, PeriodComputationResult> {
        let ExtendedERpmData::Rpm { shift, base } = self.data else {
            return Err(PeriodComputationResult::NotRpmPacket)
        };

        if base == 0 {
            return Err(PeriodComputationResult::StoppedMotor);
        }

        let period = (base as u32) << shift;
        NonZeroU32::new(period).ok_or(PeriodComputationResult::StoppedMotor)
    }

    /// Computes [`u32`] motor Rpm.
    /// 
    /// Returns [`PeriodComputationResult::NotRpmPacket`] when the packet type is not RPM.
    pub fn compute_rpm(&self) -> Result<u32, PeriodComputationResult> {
        match self.compute_period_us() {
            Ok(period) => Ok(60_000_000 / period),
            Err(PeriodComputationResult::StoppedMotor) => Ok(0),
            Err(e) => Err(e)
        }
    }

    /// Returns internal 3 bit [`u8`] period_us shift value.
    /// 
    /// Returns [`None`] when the packet is not of type Rpm.
    pub fn shift(&self) -> Option<u8> {
        match self.data {
            ExtendedERpmData::Rpm { shift, .. } => Some(shift),
            _ => None
        }
    }
    
    /// Returns internal 9 bit [`u16`] period_us base value.
    /// 
    /// Returns [`None`] when the packet is not of type Rpm.
    pub fn base(&self) -> Option<u16> {
        match self.data {
            ExtendedERpmData::Rpm { base, .. } => Some(base),
            _ => None
        }
    }
}