//! Stable typed operating catalog; no native flags or CAT strings leave this adapter.

use crate::{Error, Rig, raw, rig::status};
use std::ffi::CStr;

/// One exact integer-valued setting. Normalized controls use millionths.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Setting {
    /// Receive frequency in hertz.
    ReceiveFrequency = 1,
    /// Absolute split transmit frequency in hertz.
    TransmitFrequency = 2,
    /// Shared mode selector, FM=1 through packet LSB=12.
    Mode = 3,
    /// Passband width in hertz; zero requests normal bandwidth.
    Bandwidth = 4,
    /// Receive VFO: current=0, A=1, B=2, main=3, sub=4, memory=5.
    ReceiveVfo = 5,
    /// Transmit VFO, using the same selectors.
    TransmitVfo = 6,
    /// Split enable, independent of repeater shift.
    Split = 7,
    /// Signed repeater shift in hertz.
    Offset = 8,
    /// Receive incremental tuning in hertz.
    Rit = 9,
    /// Transmit incremental tuning in hertz.
    Xit = 10,
    /// RF power in milliwatts, converted by the rig's power curve.
    Power = 11,
    /// Transmit CTCSS in tenths of hertz; zero disables.
    TransmitCtcss = 12,
    /// Receive CTCSS in tenths of hertz; zero disables.
    ReceiveCtcss = 13,
    /// Transmit DCS as an actual nine-bit value, not printed octal digits.
    TransmitDcs = 14,
    /// Receive DCS as an actual nine-bit value.
    ReceiveDcs = 15,
    /// CTCSS encoder enable.
    TransmitToneEnabled = 16,
    /// CTCSS decoder enable.
    ReceiveToneEnabled = 17,
    /// PTT; accessible only through the selected owner and RF safety policy.
    Ptt = 20,
    /// Rig carrier-detect observation.
    CarrierDetect = 21,
    /// Transmit mode for split-VFO operation.
    SplitMode = 22,
    /// Transmit passband for split-VFO operation, in hertz.
    SplitBandwidth = 23,
    /// AF gain in millionths of the rig control's full scale.
    AfGain = 100,
    /// RF gain in millionths.
    RfGain = 101,
    /// Microphone gain in millionths.
    MicrophoneGain = 102,
    /// Squelch threshold in millionths.
    Squelch = 103,
    /// Compression level in millionths.
    Compression = 104,
    /// AGC mode using Hamlib's documented small enum, not a bit mask.
    Agc = 105,
    /// IF shift in hertz.
    IfShift = 106,
    /// Preamp gain in decibels, from the rig's discrete choices.
    Preamp = 107,
    /// Attenuation in decibels, from the rig's discrete choices.
    Attenuator = 108,
    /// Noise reduction level in millionths.
    NoiseReductionLevel = 109,
    /// Notch frequency in hertz.
    NotchFrequency = 110,
    /// Keyer speed in words per minute.
    KeyerSpeed = 111,
    /// Antenna number, one-based.
    Antenna = 200,
    /// Noise blanker enable.
    NoiseBlanker = 300,
    /// Noise reduction enable.
    NoiseReduction = 301,
    /// Automatic notch enable.
    AutomaticNotch = 302,
    /// Manual notch enable.
    ManualNotch = 303,
    /// Compressor enable.
    Compressor = 304,
    /// Automatic antenna tuner enable.
    Tuner = 305,
    /// Monitor enable.
    Monitor = 306,
    /// RIT enable.
    RitEnabled = 307,
    /// XIT enable.
    XitEnabled = 308,
}

/// Scalar interpretation, independent of native flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ValueKind {
    /// Exactly zero or one; negative zero is not a boolean input.
    Boolean = 1,
    /// Nonnegative integral value in the declared units.
    Unsigned = 2,
    /// Signed integral value in the declared units.
    Signed = 3,
    /// Fixed-point value using the declared decimal precision.
    Decimal = 4,
    /// One declared discrete selector, never a native bit mask.
    Enumeration = 5,
}

/// Units used to format and validate a value; normalized controls are not dB.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum ValueUnit {
    /// Dimensionless boolean or enum selector.
    None = 0,
    /// Hertz, with decimal precision for CTCSS.
    Hertz = 1,
    /// Milliwatts.
    Milliwatts = 2,
    /// Actual nine-bit DCS code, conventionally displayed as three octal digits.
    DcsCode = 3,
    /// Decibels.
    Decibels = 4,
    /// Words per minute.
    WordsPerMinute = 5,
    /// Dimensionless full-scale ratio, commonly represented in millionths.
    Ratio = 6,
}

/// Complete non-native catalog entry, copied while the owner has exclusive access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingInfo {
    /// Stable adapter-neutral selector.
    pub setting: Setting,
    /// Configuration/status name, independent of native flags.
    pub name: &'static str,
    /// True when an actual getter exists.
    pub readable: bool,
    /// True when an actual setter exists.
    pub writable: bool,
    /// Scalar value kind, separate from access and precision.
    pub value_kind: ValueKind,
    /// Physical/control units.
    pub units: ValueUnit,
    /// Decimal places represented by the integer value.
    pub decimal_places: u32,
    /// Inclusive lower bound.
    pub minimum: i64,
    /// Inclusive upper bound.
    pub maximum: i64,
    /// Nonempty list restricts values to these exact choices.
    pub choices: Vec<i64>,
}

#[derive(Clone, Copy)]
enum Native {
    Operation(raw::rig_function_e, raw::rig_function_e),
    Level(&'static CStr, bool),
    Function(&'static CStr),
}

pub(crate) fn operation_capabilities(
    setting: Setting,
    read: raw::rig_function_e,
    write: raw::rig_function_e,
    has: impl Fn(raw::rig_function_e) -> bool,
) -> (bool, bool) {
    let readable = read != u32::MAX && has(read);
    let writable = write != u32::MAX && has(write);
    if setting == Setting::Offset {
        // A shift without its magnitude (or vice versa) is not an offset control.
        (
            readable && has(raw::RIG_FUNCTION_GET_RPTR_SHIFT),
            writable && has(raw::RIG_FUNCTION_SET_RPTR_SHIFT),
        )
    } else {
        (readable, writable)
    }
}

/// One negotiated rig coverage interval; masks reserve bit zero and shift by selector.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Coverage {
    /// Inclusive integer-hertz lower bound.
    pub minimum_hz: u64,
    /// Inclusive integer-hertz upper bound.
    pub maximum_hz: u64,
    /// Supported shared modes for this interval.
    pub modes: u64,
    /// Native minimum power in milliwatts; negative means unspecified.
    pub minimum_power_mw: i32,
    /// Native maximum power in milliwatts; negative means unspecified.
    pub maximum_power_mw: i32,
}

const MODES: [&CStr; 12] = [
    c"FM", c"AM", c"USB", c"LSB", c"CW", c"CWR", c"RTTY", c"RTTYR", c"WFM", c"PKTFM", c"PKTUSB",
    c"PKTLSB",
];
const VFOS: [&CStr; 6] = [c"currVFO", c"VFOA", c"VFOB", c"Main", c"Sub", c"MEM"];
const SETTINGS: &[Setting] = &[
    Setting::ReceiveFrequency,
    Setting::TransmitFrequency,
    Setting::Mode,
    Setting::Bandwidth,
    Setting::ReceiveVfo,
    Setting::TransmitVfo,
    Setting::Split,
    Setting::Offset,
    Setting::Rit,
    Setting::Xit,
    Setting::Power,
    Setting::TransmitCtcss,
    Setting::ReceiveCtcss,
    Setting::TransmitDcs,
    Setting::ReceiveDcs,
    Setting::TransmitToneEnabled,
    Setting::ReceiveToneEnabled,
    Setting::Ptt,
    Setting::CarrierDetect,
    Setting::SplitMode,
    Setting::SplitBandwidth,
    Setting::AfGain,
    Setting::RfGain,
    Setting::MicrophoneGain,
    Setting::Squelch,
    Setting::Compression,
    Setting::Agc,
    Setting::IfShift,
    Setting::Preamp,
    Setting::Attenuator,
    Setting::NoiseReductionLevel,
    Setting::NotchFrequency,
    Setting::KeyerSpeed,
    Setting::Antenna,
    Setting::NoiseBlanker,
    Setting::NoiseReduction,
    Setting::AutomaticNotch,
    Setting::ManualNotch,
    Setting::Compressor,
    Setting::Tuner,
    Setting::Monitor,
    Setting::RitEnabled,
    Setting::XitEnabled,
];

impl Setting {
    fn value_type(self) -> (ValueKind, ValueUnit) {
        use ValueKind as K;
        use ValueUnit as U;
        match self {
            Self::ReceiveFrequency
            | Self::TransmitFrequency
            | Self::Bandwidth
            | Self::SplitBandwidth
            | Self::NotchFrequency => (K::Unsigned, U::Hertz),
            Self::Offset | Self::Rit | Self::Xit | Self::IfShift => (K::Signed, U::Hertz),
            Self::Power => (K::Unsigned, U::Milliwatts),
            Self::TransmitCtcss | Self::ReceiveCtcss => (K::Decimal, U::Hertz),
            Self::TransmitDcs | Self::ReceiveDcs => (K::Unsigned, U::DcsCode),
            Self::Mode
            | Self::SplitMode
            | Self::ReceiveVfo
            | Self::TransmitVfo
            | Self::Agc
            | Self::Antenna => (K::Enumeration, U::None),
            Self::Preamp | Self::Attenuator => (K::Unsigned, U::Decibels),
            Self::KeyerSpeed => (K::Unsigned, U::WordsPerMinute),
            Self::AfGain
            | Self::RfGain
            | Self::MicrophoneGain
            | Self::Squelch
            | Self::Compression
            | Self::NoiseReductionLevel => (K::Decimal, U::Ratio),
            Self::Split
            | Self::TransmitToneEnabled
            | Self::ReceiveToneEnabled
            | Self::Ptt
            | Self::CarrierDetect
            | Self::NoiseBlanker
            | Self::NoiseReduction
            | Self::AutomaticNotch
            | Self::ManualNotch
            | Self::Compressor
            | Self::Tuner
            | Self::Monitor
            | Self::RitEnabled
            | Self::XitEnabled => (K::Boolean, U::None),
        }
    }
    /// Reject unknown selectors rather than exposing native enum values.
    pub fn from_selector(selector: u32) -> Result<Self, Error> {
        SETTINGS
            .iter()
            .copied()
            .find(|item| *item as u32 == selector)
            .ok_or(Error::InvalidState)
    }

    fn native(self) -> Native {
        use Native::{Function as F, Level as L, Operation as O};
        use raw::*;
        match self {
            Self::ReceiveFrequency => O(RIG_FUNCTION_GET_FREQ, RIG_FUNCTION_SET_FREQ),
            Self::TransmitFrequency => O(RIG_FUNCTION_GET_SPLIT_FREQ, RIG_FUNCTION_SET_SPLIT_FREQ),
            Self::Mode | Self::Bandwidth => O(RIG_FUNCTION_GET_MODE, RIG_FUNCTION_SET_MODE),
            Self::SplitMode | Self::SplitBandwidth => O(u32::MAX, RIG_FUNCTION_SET_SPLIT_MODE),
            Self::ReceiveVfo => O(RIG_FUNCTION_GET_VFO, RIG_FUNCTION_SET_VFO),
            Self::TransmitVfo | Self::Split => {
                O(RIG_FUNCTION_GET_SPLIT_VFO, RIG_FUNCTION_SET_SPLIT_VFO)
            }
            Self::Offset => O(RIG_FUNCTION_GET_RPTR_OFFS, RIG_FUNCTION_SET_RPTR_OFFS),
            Self::Rit => O(RIG_FUNCTION_GET_RIT, RIG_FUNCTION_SET_RIT),
            Self::Xit => O(RIG_FUNCTION_GET_XIT, RIG_FUNCTION_SET_XIT),
            Self::Power => L(c"RFPOWER", true),
            Self::TransmitCtcss => O(RIG_FUNCTION_GET_CTCSS_TONE, RIG_FUNCTION_SET_CTCSS_TONE),
            Self::ReceiveCtcss => O(RIG_FUNCTION_GET_CTCSS_SQL, RIG_FUNCTION_SET_CTCSS_SQL),
            Self::TransmitDcs => O(RIG_FUNCTION_GET_DCS_CODE, RIG_FUNCTION_SET_DCS_CODE),
            Self::ReceiveDcs => O(RIG_FUNCTION_GET_DCS_SQL, RIG_FUNCTION_SET_DCS_SQL),
            Self::TransmitToneEnabled => F(c"TONE"),
            Self::ReceiveToneEnabled => F(c"TSQL"),
            Self::Ptt => O(RIG_FUNCTION_GET_PTT, RIG_FUNCTION_SET_PTT),
            Self::CarrierDetect => O(RIG_FUNCTION_GET_DCD, u32::MAX),
            Self::AfGain => L(c"AF", true),
            Self::RfGain => L(c"RF", true),
            Self::MicrophoneGain => L(c"MICGAIN", true),
            Self::Squelch => L(c"SQL", true),
            Self::Compression => L(c"COMP", true),
            Self::Agc => L(c"AGC", false),
            Self::IfShift => L(c"IF", false),
            Self::Preamp => L(c"PREAMP", false),
            Self::Attenuator => L(c"ATT", false),
            Self::NoiseReductionLevel => L(c"NR", true),
            Self::NotchFrequency => L(c"NOTCHF", false),
            Self::KeyerSpeed => L(c"KEYSPD", false),
            Self::Antenna => O(RIG_FUNCTION_GET_ANT, RIG_FUNCTION_SET_ANT),
            Self::NoiseBlanker => F(c"NB"),
            Self::NoiseReduction => F(c"NR"),
            Self::AutomaticNotch => F(c"ANF"),
            Self::ManualNotch => F(c"MN"),
            Self::Compressor => F(c"COMP"),
            Self::Tuner => F(c"TUNER"),
            Self::Monitor => F(c"MON"),
            Self::RitEnabled => F(c"RIT"),
            Self::XitEnabled => F(c"XIT"),
        }
    }
}

/// Translate actual nine-bit DCS to Hamlib's decimal-spelled octal digits.
pub fn dcs_to_native(code: u32) -> Result<u32, Error> {
    if code > 0x1ff {
        return Err(Error::InvalidState);
    }
    Ok((code >> 6) * 100 + ((code >> 3) & 7) * 10 + (code & 7))
}

/// Validate all three octal digits before rebuilding the actual nine bits.
pub fn dcs_from_native(code: u32) -> Result<u32, Error> {
    let (a, b, c) = (code / 100, (code / 10) % 10, code % 10);
    if a > 7 || b > 7 || c > 7 {
        return Err(Error::InvalidState);
    }
    Ok((a << 6) | (b << 3) | c)
}

fn native_mode(selector: i64) -> Result<u64, Error> {
    let name = MODES
        .get(usize::try_from(selector - 1).map_err(|_| Error::InvalidState)?)
        .ok_or(Error::InvalidState)?;
    // SAFETY: static NUL-terminated mode name from the closed shared enum.
    Ok(unsafe { raw::rig_parse_mode(name.as_ptr()) })
}

fn mode_selector(mode: u64) -> Result<i64, Error> {
    (1..=MODES.len() as i64)
        .find(|selector| native_mode(*selector) == Ok(mode))
        .ok_or(Error::InvalidState)
}

fn native_vfo(selector: i64) -> Result<u32, Error> {
    let name = VFOS
        .get(usize::try_from(selector).map_err(|_| Error::InvalidState)?)
        .ok_or(Error::InvalidState)?;
    // SAFETY: static NUL-terminated name from the closed VFO enum.
    Ok(unsafe { raw::rig_parse_vfo(name.as_ptr()) })
}

fn vfo_selector(vfo: u32) -> Result<i64, Error> {
    (0..VFOS.len() as i64)
        .find(|selector| native_vfo(*selector) == Ok(vfo))
        .ok_or(Error::InvalidState)
}

pub(crate) fn tone_choices(pointer: *const u32, dcs: bool) -> Result<Vec<i64>, Error> {
    let mut choices = vec![0];
    if pointer.is_null() {
        return Ok(Vec::new());
    }
    // SAFETY: Hamlib owns a zero-terminated native tone table for this live rig.
    // The bounded walk rejects malformed metadata rather than looping forever.
    for index in 0..128 {
        let value = unsafe { pointer.add(index).read() };
        if value == 0 {
            return Ok(choices);
        }
        choices.push(i64::from(if dcs { dcs_from_native(value)? } else { value }));
    }
    Err(Error::InvalidState)
}

impl Rig {
    /// Copy actual receive/transmit ranges after this connection's negotiation.
    pub fn coverage(&self, transmit: bool) -> Result<Vec<Coverage>, Error> {
        let state = self.native_state()?;
        let ranges = if transmit {
            &state.tx_range_list
        } else {
            &state.rx_range_list
        };
        let mut result = Vec::new();
        for range in ranges.iter().take_while(|range| range.endf != 0.0) {
            if !range.startf.is_finite()
                || !range.endf.is_finite()
                || range.startf < 0.0
                || range.endf < range.startf
                || range.endf > (1_u64 << 53) as f64
            {
                return Err(Error::InvalidState);
            }
            let modes = (1..=12)
                .filter(|selector| native_mode(*selector).is_ok_and(|mode| range.modes & mode != 0))
                .fold(0, |bits, selector| bits | (1 << selector));
            result.push(Coverage {
                minimum_hz: range.startf.ceil() as u64,
                maximum_hz: range.endf.floor() as u64,
                modes,
                minimum_power_mw: range.low_power,
                maximum_power_mw: range.high_power,
            });
        }
        Ok(result)
    }

    /// Runtime per-connection capabilities, including NETRIGCTL's negotiated state.
    fn native_state(&self) -> Result<&raw::rig_state, Error> {
        // SAFETY: public Hamlib getter returns this exclusively owned handle's
        // state. Its layout is generated from the same installed headers.
        let state = unsafe { raw::rig_data_pointer(self.pointer()?, raw::RIG_PTRX_STATE) }
            .cast::<raw::rig_state>();
        unsafe { state.as_ref() }.ok_or(Error::InvalidState)
    }

    /// A real backend or selected hardware-line getter is required; the
    /// frontend's cached transmit intent is not a physical PTT observation.
    pub(crate) fn ptt_observable(&self) -> Result<bool, Error> {
        // SAFETY: this is the selected port owned by the live rig; configuration
        // updates this union, not rig_state's legacy ptt_type field.
        let port = unsafe { raw::rig_data_pointer(self.pointer()?, raw::RIG_PTRX_PTTPORT) }
            .cast::<raw::hamlib_port_t>();
        // SAFETY: a live initialized rig owns a complete public port record.
        let port = unsafe { port.as_ref() }.ok_or(Error::InvalidState)?;
        // SAFETY: PTTPORT selects the ptt member of the public port union.
        let selected = unsafe { port.type_.ptt };
        Ok(match selected {
            raw::RIG_PTT_RIG | raw::RIG_PTT_RIG_MICDATA => {
                // SAFETY: immutable model capabilities outlive this rig owner.
                unsafe { raw::rig_get_caps(self.model).as_ref() }
                    .is_some_and(|caps| caps.get_ptt.is_some())
            }
            // A closed shared serial PTT port makes Hamlib assume OFF.
            raw::RIG_PTT_SERIAL_DTR | raw::RIG_PTT_SERIAL_RTS => port.fd >= 0,
            raw::RIG_PTT_PARALLEL => true,
            _ => false,
        })
    }

    /// Copy available typed controls. Unsupported operations are absent, not cached success.
    pub fn catalog(&self) -> Result<Vec<SettingInfo>, Error> {
        let handle = self.pointer()?;
        let state = self.native_state()?;
        let mut result = Vec::new();
        for setting in SETTINGS.iter().copied() {
            let (readable, writable, normalized) = match setting.native() {
                Native::Operation(read, write) => {
                    // SAFETY: model comes from installed headers. These pointers
                    // are capability hints only; every operation checks its result.
                    let has = |operation| {
                        operation != u32::MAX
                            && !unsafe { raw::rig_get_function_ptr(self.model, operation) }
                                .is_null()
                    };
                    let (mut readable, writable) =
                        operation_capabilities(setting, read, write, has);
                    if setting == Setting::Ptt {
                        readable = self.ptt_observable()?;
                    }
                    if matches!(setting, Setting::SplitMode | Setting::SplitBandwidth) {
                        // Hamlib 4.6 has no GET_SPLIT_MODE function selector.
                        // SAFETY: the loaded model's immutable public caps live with Hamlib.
                        readable = unsafe { raw::rig_get_caps(self.model).as_ref() }
                            .is_some_and(|caps| {
                                (caps.get_split_mode.is_some() || caps.get_mode.is_some())
                                    && (caps.targetable_vfo & raw::RIG_TARGETABLE_MODE as i32 != 0
                                        || caps.set_vfo.is_some()
                                        || caps.vfo_op.is_some()
                                            // SAFETY: the retained live owner only queries capabilities.
                                            && unsafe { raw::rig_has_vfo_op(handle, raw::RIG_OP_TOGGLE) } != 0)
                            });
                    }
                    (readable, writable, false)
                }
                Native::Level(name, normalized) => {
                    let level = unsafe { raw::rig_parse_level(name.as_ptr()) };
                    (
                        level != 0 && state.has_get_level & level != 0,
                        level != 0 && state.has_set_level & level != 0,
                        normalized && setting != Setting::Power,
                    )
                }
                Native::Function(name) => {
                    let function = unsafe { raw::rig_parse_func(name.as_ptr()) };
                    (
                        function != 0 && state.has_get_func & function != 0,
                        function != 0 && state.has_set_func & function != 0,
                        false,
                    )
                }
            };
            if !readable && !writable {
                continue;
            }
            let (mut minimum, mut maximum) = match setting {
                Setting::ReceiveFrequency | Setting::TransmitFrequency => (1, 1_i64 << 53),
                Setting::Mode | Setting::SplitMode => (1, 12),
                Setting::ReceiveVfo | Setting::TransmitVfo => (0, 5),
                Setting::Bandwidth | Setting::SplitBandwidth => (0, i32::MAX as i64),
                Setting::Rit => (-state.max_rit, state.max_rit),
                Setting::Xit => (-state.max_xit, state.max_xit),
                Setting::Power => (0, u32::MAX as i64),
                Setting::TransmitDcs | Setting::ReceiveDcs => (0, 511),
                Setting::TransmitCtcss | Setting::ReceiveCtcss => (0, 3000),
                Setting::Offset | Setting::IfShift => (-(i32::MAX as i64), i32::MAX as i64),
                Setting::Antenna => (1, 31),
                _ if normalized => (0, 1_000_000),
                _ if matches!(setting.native(), Native::Function(_))
                    || matches!(
                        setting,
                        Setting::Ptt | Setting::CarrierDetect | Setting::Split
                    ) =>
                {
                    (0, 1)
                }
                _ => (0, i32::MAX as i64),
            };
            if let Native::Level(name, normalized) = setting.native() {
                let level = unsafe { raw::rig_parse_level(name.as_ptr()) };
                let index = unsafe { raw::rig_setting2idx(level) };
                if let Some(granularity) = usize::try_from(index)
                    .ok()
                    .and_then(|index| state.level_gran.get(index))
                {
                    // SAFETY: the catalog fixes each native level's active union member.
                    let (low, high) = unsafe {
                        if normalized {
                            (granularity.min.f as f64, granularity.max.f as f64)
                        } else {
                            (granularity.min.i as f64, granularity.max.i as f64)
                        }
                    };
                    if setting != Setting::Power
                        && low.is_finite()
                        && high.is_finite()
                        && high > low
                    {
                        let scale = if normalized { 1_000_000.0 } else { 1.0 };
                        minimum = (low * scale).ceil() as i64;
                        maximum = (high * scale).floor() as i64;
                    }
                }
            }
            let choices = match setting {
                Setting::Mode | Setting::SplitMode => (1..=12)
                    .filter(|value| {
                        native_mode(*value).is_ok_and(|mode| state.mode_list & mode != 0)
                    })
                    .collect(),
                Setting::ReceiveVfo | Setting::TransmitVfo => std::iter::once(0)
                    .chain((1..=5).filter(|value| {
                        native_vfo(*value).is_ok_and(|vfo| state.vfo_list as u32 & vfo != 0)
                    }))
                    .collect(),
                Setting::TransmitCtcss | Setting::ReceiveCtcss => {
                    tone_choices(state.ctcss_list, false)?
                }
                Setting::TransmitDcs | Setting::ReceiveDcs => tone_choices(state.dcs_list, true)?,
                Setting::Agc => state
                    .agc_levels
                    .iter()
                    .take(
                        usize::try_from(state.agc_level_count)
                            .unwrap_or(0)
                            .min(state.agc_levels.len()),
                    )
                    .copied()
                    .map(i64::from)
                    .collect(),
                Setting::Preamp => std::iter::once(0)
                    .chain(
                        state
                            .preamp
                            .iter()
                            .copied()
                            .take_while(|value| *value != 0)
                            .map(i64::from),
                    )
                    .collect(),
                Setting::Attenuator => std::iter::once(0)
                    .chain(
                        state
                            .attenuator
                            .iter()
                            .copied()
                            .take_while(|value| *value != 0)
                            .map(i64::from),
                    )
                    .collect(),
                _ => Vec::new(),
            };
            let name = match setting.native() {
                Native::Level(name, _) | Native::Function(name) => {
                    name.to_str().map_err(|_| Error::InvalidState)?
                }
                _ => match setting {
                    Setting::ReceiveFrequency => "receive_hz",
                    Setting::TransmitFrequency => "transmit_hz",
                    Setting::Mode => "mode",
                    Setting::Bandwidth => "bandwidth_hz",
                    Setting::SplitMode => "split_mode",
                    Setting::SplitBandwidth => "split_bandwidth_hz",
                    Setting::ReceiveVfo => "receive_vfo",
                    Setting::TransmitVfo => "transmit_vfo",
                    Setting::Split => "split",
                    Setting::Offset => "offset_hz",
                    Setting::Rit => "rit_hz",
                    Setting::Xit => "xit_hz",
                    Setting::TransmitCtcss => "encode_ctcss",
                    Setting::ReceiveCtcss => "decode_ctcss",
                    Setting::TransmitDcs => "encode_dcs",
                    Setting::ReceiveDcs => "decode_dcs",
                    Setting::Ptt => "ptt",
                    Setting::CarrierDetect => "carrier_detect",
                    Setting::Antenna => "antenna",
                    _ => unreachable!(),
                },
            };
            result.push(SettingInfo {
                setting,
                name,
                readable,
                writable,
                value_kind: setting.value_type().0,
                units: setting.value_type().1,
                decimal_places: if normalized {
                    6
                } else if matches!(setting, Setting::TransmitCtcss | Setting::ReceiveCtcss) {
                    1
                } else {
                    0
                },
                minimum,
                maximum,
                choices,
            });
        }
        Ok(result)
    }

    fn validate_value(&self, setting: Setting, value: i64) -> Result<(), Error> {
        let info = self
            .catalog()?
            .into_iter()
            .find(|item| item.setting == setting)
            .ok_or(Error::InvalidState)?;
        if !info.writable
            || value < info.minimum
            || value > info.maximum
            || (!info.choices.is_empty() && !info.choices.contains(&value))
        {
            return Err(Error::InvalidState);
        }
        Ok(())
    }

    /// Set mode/passband from complete desired state, without requiring readback.
    pub fn set_mode_width(&mut self, mode: i64, width: u32) -> Result<(), Error> {
        self.validate_value(Setting::Mode, mode)?;
        self.validate_value(Setting::Bandwidth, i64::from(width))?;
        let handle = self.pointer()?;
        let mode = native_mode(mode)?;
        // SAFETY: this rig has one owner; values are admitted by its catalog.
        unsafe {
            let width = if width == 0 {
                raw::rig_passband_normal(handle, mode)
            } else {
                i64::from(width)
            };
            status(raw::rig_set_mode(
                handle,
                raw::RPTADV_VFO_CURRENT,
                mode,
                width,
            ))
        }
    }

    /// Set split routing from complete desired VFO state, without a getter.
    pub fn set_split_vfos(
        &mut self,
        receive: i64,
        enabled: bool,
        transmit: i64,
    ) -> Result<(), Error> {
        self.validate_value(Setting::ReceiveVfo, receive)?;
        self.validate_value(Setting::TransmitVfo, transmit)?;
        self.validate_value(Setting::Split, i64::from(enabled))?;
        // SAFETY: native VFO selectors are validated and the rig is uniquely owned.
        status(unsafe {
            raw::rig_set_split_vfo(
                self.pointer()?,
                native_vfo(receive)?,
                u32::from(enabled),
                native_vfo(transmit)?,
            )
        })
    }

    /// Set the split transmit mode only after the owner has safely unkeyed and
    /// enabled the intended split routing. Hamlib may otherwise enable split or
    /// skip a mode write while PTT is active.
    pub fn set_split_mode_width(&mut self, mode: i64, width: u32) -> Result<(), Error> {
        self.validate_value(Setting::SplitMode, mode)?;
        self.validate_value(Setting::SplitBandwidth, i64::from(width))?;
        let handle = self.pointer()?;
        let mode = native_mode(mode)?;
        // SAFETY: the unique physical owner supplies an admitted mode/passband.
        unsafe {
            let width = if width == 0 {
                raw::rig_passband_normal(handle, mode)
            } else {
                i64::from(width)
            };
            status(raw::rig_set_split_mode(
                handle,
                raw::RPTADV_VFO_CURRENT,
                mode,
                width,
            ))
        }
    }

    /// Read absolute power from the actual RFPOWER control using effective TX
    /// frequency/mode supplied by the owner, which may differ from its RX VFO.
    pub fn get_power_mw(&mut self, transmit_hz: u64, mode: i64) -> Result<u32, Error> {
        self.fresh_read(|owner| owner.get_power_native(transmit_hz, mode))
    }

    fn get_power_native(&mut self, transmit_hz: u64, mode: i64) -> Result<u32, Error> {
        if transmit_hz == 0
            || transmit_hz > 1_u64 << 53
            || !self
                .catalog()?
                .iter()
                .any(|entry| entry.setting == Setting::Power && entry.readable)
        {
            return Err(Error::InvalidState);
        }
        let handle = self.pointer()?;
        let mode = native_mode(mode)?;
        let (mut value, mut power) = (raw::value_t { f: 0.0 }, 0);
        // SAFETY: the sole owner retains the rig; all outputs are complete stack values.
        unsafe {
            status(raw::rig_get_level(
                handle,
                raw::RPTADV_VFO_CURRENT,
                raw::rig_parse_level(c"RFPOWER".as_ptr()),
                &mut value,
            ))?;
            status(raw::rig_power2mW(
                handle,
                &mut power,
                value.f,
                transmit_hz as f64,
                mode,
            ))?;
        }
        Ok(power)
    }

    /// Convert absolute RF power using the caller's effective TX frequency/mode.
    /// This does not fabricate readback or use the possibly different RX frequency.
    pub fn set_power_mw(&mut self, power: u32, transmit_hz: u64, mode: i64) -> Result<(), Error> {
        self.validate_value(Setting::Power, i64::from(power))?;
        if transmit_hz == 0 || transmit_hz > 1_u64 << 53 {
            return Err(Error::InvalidState);
        }
        let handle = self.pointer()?;
        let mode = native_mode(mode)?;
        let mut normalized = 0.0;
        // SAFETY: the conversion and write use one retained rig and stack output.
        unsafe {
            status(raw::rig_mW2power(
                handle,
                &mut normalized,
                power,
                transmit_hz as f64,
                mode,
            ))?;
            status(raw::rig_set_level(
                handle,
                raw::RPTADV_VFO_CURRENT,
                raw::rig_parse_level(c"RFPOWER".as_ptr()),
                raw::value_t { f: normalized },
            ))
        }
    }

    /// Set one scalar. Mode/passband, split and power require readable companion
    /// state; complete-state callers use the grouped operations for write-only rigs.
    pub fn set(&mut self, setting: Setting, value: i64) -> Result<(), Error> {
        self.validate_value(setting, value)?;
        let handle = self.pointer()?;
        let vfo = raw::RPTADV_VFO_CURRENT;
        // SAFETY: all calls use the sole owner and native values validated above;
        // stack out-parameters are complete, and no pointer escapes this call.
        unsafe {
            match setting.native() {
                Native::Level(name, normalized) => {
                    let level = raw::rig_parse_level(name.as_ptr());
                    let native = if setting == Setting::Power {
                        let mut power = 0.0;
                        let (mut mode, mut width) = (0, 0);
                        status(raw::rig_get_mode(handle, vfo, &mut mode, &mut width))?;
                        status(raw::rig_mW2power(
                            handle,
                            &mut power,
                            value as u32,
                            self.frequency()? as f64,
                            mode,
                        ))?;
                        raw::value_t { f: power }
                    } else if normalized {
                        raw::value_t {
                            f: value as f32 / 1_000_000.0,
                        }
                    } else {
                        raw::value_t { i: value as i32 }
                    };
                    status(raw::rig_set_level(handle, vfo, level, native))
                }
                Native::Function(name) => status(raw::rig_set_func(
                    handle,
                    vfo,
                    raw::rig_parse_func(name.as_ptr()),
                    value as i32,
                )),
                Native::Operation(_, _) => match setting {
                    Setting::ReceiveFrequency => self.set_frequency(value as u64),
                    Setting::TransmitFrequency => {
                        status(raw::rig_set_split_freq(handle, vfo, value as f64))
                    }
                    Setting::Mode | Setting::Bandwidth => {
                        let (mut mode, mut width) = (0, 0);
                        status(raw::rig_get_mode(handle, vfo, &mut mode, &mut width))?;
                        status(raw::rig_set_mode(
                            handle,
                            vfo,
                            if setting == Setting::Mode {
                                native_mode(value)?
                            } else {
                                mode
                            },
                            if setting == Setting::Bandwidth {
                                value as _
                            } else {
                                width
                            },
                        ))
                    }
                    Setting::SplitMode | Setting::SplitBandwidth => {
                        let (mut mode, mut width) = (0, 0);
                        status(raw::rig_get_split_mode(handle, vfo, &mut mode, &mut width))?;
                        status(raw::rig_set_split_mode(
                            handle,
                            vfo,
                            if setting == Setting::SplitMode {
                                native_mode(value)?
                            } else {
                                mode
                            },
                            if setting == Setting::SplitBandwidth {
                                value as _
                            } else {
                                width
                            },
                        ))
                    }
                    Setting::ReceiveVfo => status(raw::rig_set_vfo(handle, native_vfo(value)?)),
                    Setting::TransmitVfo | Setting::Split => {
                        let (mut split, mut transmit_vfo) = (0, 0);
                        status(raw::rig_get_split_vfo(
                            handle,
                            vfo,
                            &mut split,
                            &mut transmit_vfo,
                        ))?;
                        status(raw::rig_set_split_vfo(
                            handle,
                            vfo,
                            if setting == Setting::Split {
                                value as _
                            } else {
                                split
                            },
                            if setting == Setting::TransmitVfo {
                                native_vfo(value)?
                            } else {
                                transmit_vfo
                            },
                        ))
                    }
                    Setting::Offset => {
                        status(raw::rig_set_rptr_shift(
                            handle,
                            vfo,
                            match value.cmp(&0) {
                                std::cmp::Ordering::Equal => raw::RIG_RPT_SHIFT_NONE,
                                std::cmp::Ordering::Less => raw::RIG_RPT_SHIFT_MINUS,
                                std::cmp::Ordering::Greater => raw::RIG_RPT_SHIFT_PLUS,
                            },
                        ))?;
                        status(raw::rig_set_rptr_offs(handle, vfo, value.abs() as _))
                    }
                    Setting::Rit => status(raw::rig_set_rit(handle, vfo, value as _)),
                    Setting::Xit => status(raw::rig_set_xit(handle, vfo, value as _)),
                    Setting::TransmitCtcss => {
                        status(raw::rig_set_ctcss_tone(handle, vfo, value as _))
                    }
                    Setting::ReceiveCtcss => {
                        status(raw::rig_set_ctcss_sql(handle, vfo, value as _))
                    }
                    Setting::TransmitDcs => status(raw::rig_set_dcs_code(
                        handle,
                        vfo,
                        dcs_to_native(value as u32)?,
                    )),
                    Setting::ReceiveDcs => status(raw::rig_set_dcs_sql(
                        handle,
                        vfo,
                        dcs_to_native(value as u32)?,
                    )),
                    Setting::Ptt => self.set_ptt(value != 0),
                    Setting::Antenna => status(raw::rig_set_ant(
                        handle,
                        vfo,
                        1 << (value - 1),
                        raw::value_t { i: 0 },
                    )),
                    _ => Err(Error::InvalidState),
                },
            }
        }
    }

    /// Read one actual typed scalar. Native errors remain failures, never intent echoes.
    pub fn get(&mut self, setting: Setting) -> Result<i64, Error> {
        self.fresh_read(|owner| owner.get_native(setting))
    }

    fn get_native(&mut self, setting: Setting) -> Result<i64, Error> {
        if !self
            .catalog()?
            .iter()
            .any(|item| item.setting == setting && item.readable)
        {
            return Err(Error::InvalidState);
        }
        let handle = self.pointer()?;
        let vfo = raw::RPTADV_VFO_CURRENT;
        // SAFETY: sole owner, valid closed selector, and complete stack outputs.
        unsafe {
            match setting.native() {
                Native::Level(name, normalized) => {
                    let mut value = raw::value_t { i: 0 };
                    status(raw::rig_get_level(
                        handle,
                        vfo,
                        raw::rig_parse_level(name.as_ptr()),
                        &mut value,
                    ))?;
                    if setting == Setting::Power {
                        let (mut mode, mut width, mut power) = (0, 0, 0);
                        status(raw::rig_get_mode(handle, vfo, &mut mode, &mut width))?;
                        status(raw::rig_power2mW(
                            handle,
                            &mut power,
                            value.f,
                            self.frequency()? as f64,
                            mode,
                        ))?;
                        Ok(i64::from(power))
                    } else if normalized {
                        if !value.f.is_finite() {
                            return Err(Error::InvalidState);
                        }
                        Ok((value.f as f64 * 1_000_000.0).round() as i64)
                    } else {
                        Ok(i64::from(value.i))
                    }
                }
                Native::Function(name) => {
                    let mut value = 0;
                    status(raw::rig_get_func(
                        handle,
                        vfo,
                        raw::rig_parse_func(name.as_ptr()),
                        &mut value,
                    ))?;
                    Ok(i64::from(value != 0))
                }
                Native::Operation(_, _) => match setting {
                    Setting::ReceiveFrequency => self.frequency().map(|value| value as i64),
                    Setting::TransmitFrequency => {
                        let mut value = 0.0;
                        status(raw::rig_get_split_freq(handle, vfo, &mut value))?;
                        if !value.is_finite()
                            || value < 1.0
                            || value > (1_u64 << 53) as f64
                            || value.fract() != 0.0
                        {
                            return Err(Error::InvalidState);
                        }
                        Ok(value as i64)
                    }
                    Setting::Mode | Setting::Bandwidth => {
                        let (mut mode, mut width) = (0, 0);
                        status(raw::rig_get_mode(handle, vfo, &mut mode, &mut width))?;
                        if setting == Setting::Mode {
                            mode_selector(mode)
                        } else {
                            Ok(width)
                        }
                    }
                    Setting::SplitMode | Setting::SplitBandwidth => {
                        let (mut mode, mut width) = (0, 0);
                        status(raw::rig_get_split_mode(handle, vfo, &mut mode, &mut width))?;
                        if setting == Setting::SplitMode {
                            mode_selector(mode)
                        } else {
                            Ok(width)
                        }
                    }
                    Setting::ReceiveVfo => {
                        let mut value = 0;
                        status(raw::rig_get_vfo(handle, &mut value))?;
                        vfo_selector(value)
                    }
                    Setting::TransmitVfo | Setting::Split => {
                        let (mut split, mut value) = (0, 0);
                        status(raw::rig_get_split_vfo(handle, vfo, &mut split, &mut value))?;
                        if setting == Setting::Split {
                            Ok(i64::from(split != 0))
                        } else {
                            vfo_selector(value)
                        }
                    }
                    Setting::Offset => {
                        let (mut shift, mut value) = (0, 0);
                        status(raw::rig_get_rptr_shift(handle, vfo, &mut shift))?;
                        status(raw::rig_get_rptr_offs(handle, vfo, &mut value))?;
                        Ok(if shift == raw::RIG_RPT_SHIFT_MINUS {
                            -value
                        } else if shift == raw::RIG_RPT_SHIFT_PLUS {
                            value
                        } else {
                            0
                        })
                    }
                    Setting::Rit | Setting::Xit => {
                        let mut value = 0;
                        status(if setting == Setting::Rit {
                            raw::rig_get_rit(handle, vfo, &mut value)
                        } else {
                            raw::rig_get_xit(handle, vfo, &mut value)
                        })?;
                        Ok(value)
                    }
                    Setting::TransmitCtcss
                    | Setting::ReceiveCtcss
                    | Setting::TransmitDcs
                    | Setting::ReceiveDcs => {
                        let mut value = 0;
                        status(match setting {
                            Setting::TransmitCtcss => {
                                raw::rig_get_ctcss_tone(handle, vfo, &mut value)
                            }
                            Setting::ReceiveCtcss => {
                                raw::rig_get_ctcss_sql(handle, vfo, &mut value)
                            }
                            Setting::TransmitDcs => raw::rig_get_dcs_code(handle, vfo, &mut value),
                            _ => raw::rig_get_dcs_sql(handle, vfo, &mut value),
                        })?;
                        if matches!(setting, Setting::TransmitDcs | Setting::ReceiveDcs) {
                            dcs_from_native(value).map(i64::from)
                        } else {
                            Ok(i64::from(value))
                        }
                    }
                    Setting::Ptt => self.ptt().map(i64::from),
                    Setting::CarrierDetect => {
                        let mut value = 0;
                        status(raw::rig_get_dcd(handle, vfo, &mut value))?;
                        Ok(i64::from(value != 0))
                    }
                    Setting::Antenna => {
                        let (mut option, mut current, mut transmit, mut receive) =
                            (raw::value_t { i: 0 }, 0, 0, 0);
                        status(raw::rig_get_ant(
                            handle,
                            vfo,
                            1_u32 << 31,
                            &mut option,
                            &mut current,
                            &mut transmit,
                            &mut receive,
                        ))?;
                        if current == 0 || !current.is_power_of_two() {
                            return Err(Error::InvalidState);
                        }
                        Ok(i64::from(current.trailing_zeros()) + 1)
                    }
                    _ => Err(Error::InvalidState),
                },
            }
        }
    }
}
