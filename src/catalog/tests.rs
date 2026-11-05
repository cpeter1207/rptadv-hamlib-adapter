//! Catalog admission and observed metadata use the same typed/native boundary.

use super::*;

fn dummy() -> Rig {
    Rig::open(
        "RIG_MODEL_DUMMY",
        &[("ptt_type", "RIG"), ("dcd_type", "RIG")],
    )
    .unwrap()
}

fn state(rig: &Rig) -> *mut raw::rig_state {
    // SAFETY: tests retain the sole live owner and end each mutation before an API call.
    unsafe { raw::rig_data_pointer(rig.pointer().unwrap(), raw::RIG_PTRX_STATE).cast() }
}

#[test]
fn closed_native_selectors_and_operation_capabilities_are_exact() {
    for setting in SETTINGS {
        assert_eq!(Setting::from_selector(*setting as u32), Ok(*setting));
        match setting.native() {
            Native::Level(name, _) => unsafe {
                let level = raw::rig_parse_level(name.as_ptr());
                assert_ne!(level, 0);
                assert!((0..64).contains(&raw::rig_setting2idx(level)));
            },
            Native::Function(name) => assert_ne!(unsafe { raw::rig_parse_func(name.as_ptr()) }, 0),
            _ => (),
        }
    }
    for invalid in [0, u32::MAX] {
        assert_eq!(Setting::from_selector(invalid), Err(Error::InvalidState));
    }
    for mode in 1..=12 {
        assert_eq!(mode_selector(native_mode(mode).unwrap()), Ok(mode));
    }
    for vfo in 0..=5 {
        assert_eq!(vfo_selector(native_vfo(vfo).unwrap()), Ok(vfo));
    }
    for invalid in [-1, 13] {
        assert_eq!(native_mode(invalid), Err(Error::InvalidState));
    }
    for invalid in [-1, 6] {
        assert_eq!(native_vfo(invalid), Err(Error::InvalidState));
    }
    assert_eq!(mode_selector(0), Err(Error::InvalidState));
    assert_eq!(vfo_selector(u32::MAX), Err(Error::InvalidState));
    for invalid in [800, 80, 8] {
        assert_eq!(dcs_from_native(invalid), Err(Error::InvalidState));
    }
    for setting in [Setting::Mode, Setting::Offset] {
        for read in [u32::MAX, raw::RIG_FUNCTION_GET_RPTR_OFFS] {
            for write in [u32::MAX, raw::RIG_FUNCTION_SET_RPTR_OFFS] {
                for supported in 0..16 {
                    let has = |function| match function {
                        raw::RIG_FUNCTION_GET_RPTR_OFFS => supported & 1 != 0,
                        raw::RIG_FUNCTION_SET_RPTR_OFFS => supported & 2 != 0,
                        raw::RIG_FUNCTION_GET_RPTR_SHIFT => supported & 4 != 0,
                        raw::RIG_FUNCTION_SET_RPTR_SHIFT => supported & 8 != 0,
                        _ => false,
                    };
                    assert_eq!(
                        operation_capabilities(setting, read, write, has),
                        (
                            read != u32::MAX
                                && supported & 1 != 0
                                && (setting != Setting::Offset || supported & 4 != 0),
                            write != u32::MAX
                                && supported & 2 != 0
                                && (setting != Setting::Offset || supported & 8 != 0)
                        )
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_or_unspecified_native_tone_tables_do_not_invent_choices() {
    assert_eq!(tone_choices(std::ptr::null(), false), Ok(vec![]));
    assert_eq!(tone_choices([0].as_ptr(), false), Ok(vec![0]));
    assert_eq!(tone_choices([1148, 0].as_ptr(), false), Ok(vec![0, 1148]));
    assert_eq!(tone_choices([23, 0].as_ptr(), true), Ok(vec![0, 0o23]));
    assert_eq!(
        tone_choices([888, 0].as_ptr(), true),
        Err(Error::InvalidState)
    );
    assert_eq!(
        tone_choices([1148; 128].as_ptr(), false),
        Err(Error::InvalidState)
    );
}

#[test]
fn dummy_roundtrips_remaining_operational_families() {
    let mut rig = dummy();
    rig.set_mode_width(1, 0).unwrap();
    rig.set_frequency(146_520_000).unwrap();
    rig.set_split_vfos(1, true, 2).unwrap();
    for (setting, value) in [
        (Setting::TransmitFrequency, 435_000_000),
        (Setting::SplitMode, 3),
        (Setting::SplitBandwidth, 2400),
        (Setting::Split, 0),
        (Setting::TransmitVfo, 2),
        (Setting::ReceiveVfo, 1),
        (Setting::Offset, 600_000),
        (Setting::Offset, 0),
        (Setting::Ptt, 0),
        (Setting::Antenna, 1),
    ] {
        rig.set(setting, value).unwrap();
        assert_eq!(rig.get(setting), Ok(value), "{setting:?}");
    }
    rig.set_split_mode_width(3, 0).unwrap();
    rig.set(Setting::Power, 5000).unwrap();
    assert!(rig.get(Setting::Power).unwrap() > 0);
    assert_eq!(rig.get(Setting::CarrierDetect), Ok(0));
    assert_eq!(rig.set(Setting::CarrierDetect, 1), Err(Error::InvalidState));
    assert_eq!(rig.set(Setting::Preamp, 1), Err(Error::InvalidState));
    assert_eq!(
        rig.set_power_mw(1000, (1_u64 << 53) + 1, 1),
        Err(Error::InvalidState)
    );
    assert_eq!(
        rig.get_power_mw((1_u64 << 53) + 1, 1),
        Err(Error::InvalidState)
    );
}

#[test]
fn per_connection_metadata_rejects_invalid_ranges_and_omits_unsupported_controls() {
    let mut rig = dummy();
    let state = state(&rig);
    let saved = unsafe { (*state).rx_range_list };
    for (start, end) in [
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
        (-1.0, 1.0),
        (2.0, 1.0),
        (1.0, (1_u64 << 53) as f64 + 2.0),
    ] {
        unsafe {
            (*state).rx_range_list[0].startf = start;
            (*state).rx_range_list[0].endf = end;
        }
        assert_eq!(rig.coverage(false), Err(Error::InvalidState));
    }
    unsafe {
        (*state).rx_range_list = saved;
    }
    let af = unsafe { raw::rig_setting2idx(raw::rig_parse_level(c"AF".as_ptr())) } as usize;
    let agc = unsafe { raw::rig_setting2idx(raw::rig_parse_level(c"AGC".as_ptr())) } as usize;
    for (low, high) in [
        (f32::NAN, 1.0),
        (0.0, f32::INFINITY),
        (1.0, 0.0),
        (0.1, 0.9),
    ] {
        unsafe {
            (*state).level_gran[af].min.f = low;
            (*state).level_gran[af].max.f = high;
            (*state).level_gran[agc].min.i = 0;
            (*state).level_gran[agc].max.i = 10;
        }
        let entries = rig.catalog().unwrap();
        let gain = entries
            .iter()
            .find(|entry| entry.setting == Setting::AfGain)
            .unwrap();
        assert!(gain.minimum <= gain.maximum);
    }
    unsafe {
        (*state).has_set_level = 0;
    }
    assert_eq!(rig.set(Setting::AfGain, 500_000), Err(Error::InvalidState));
    unsafe {
        (*state).has_get_level = 0;
        (*state).has_set_level = 0;
        (*state).has_get_func = 0;
        (*state).has_set_func = 0;
    }
    let entries = rig.catalog().unwrap();
    assert!(
        entries
            .iter()
            .all(|entry| !matches!(entry.setting, Setting::AfGain | Setting::Tuner))
    );
    assert_eq!(rig.get(Setting::AfGain), Err(Error::InvalidState));
    assert_eq!(rig.get_power_mw(146_520_000, 1), Err(Error::InvalidState));
    assert_eq!(rig.set(Setting::AfGain, 0), Err(Error::InvalidState));
}

#[test]
fn physical_port_and_split_vfo_capabilities_require_actual_observations() {
    let rig = dummy();
    let caps = unsafe { &*raw::rig_get_caps(rig.model) };
    let mut copy = *caps;
    for getters in 0..4 {
        for addressing in 0..16 {
            copy.get_split_mode = if getters & 1 != 0 {
                caps.get_split_mode
            } else {
                None
            };
            copy.get_mode = if getters & 2 != 0 {
                caps.get_mode
            } else {
                None
            };
            copy.targetable_vfo = if addressing & 1 != 0 {
                raw::RIG_TARGETABLE_MODE as i32
            } else {
                0
            };
            copy.set_vfo = if addressing & 2 != 0 {
                caps.set_vfo
            } else {
                None
            };
            copy.vfo_op = if addressing & 4 != 0 {
                caps.vfo_op
            } else {
                None
            };
            assert_eq!(
                split_mode_readable(&copy, addressing & 8 != 0),
                getters != 0 && (addressing & 3 != 0 || addressing & 12 == 12)
            );
        }
    }
    let mut port: raw::hamlib_port_t = unsafe { std::mem::zeroed() };
    for selected in [raw::RIG_PTT_RIG, raw::RIG_PTT_RIG_MICDATA] {
        port.type_.ptt = selected;
        assert!(port_ptt_observable(&port, rig.model));
        assert!(!port_ptt_observable(&port, 0));
    }
    for selected in [raw::RIG_PTT_SERIAL_DTR, raw::RIG_PTT_SERIAL_RTS] {
        port.type_.ptt = selected;
        port.fd = -1;
        assert!(!port_ptt_observable(&port, rig.model));
        port.fd = 0;
        assert!(port_ptt_observable(&port, rig.model));
    }
    port.type_.ptt = raw::RIG_PTT_PARALLEL;
    assert!(port_ptt_observable(&port, rig.model));
    port.type_.ptt = raw::RIG_PTT_NONE;
    assert!(!port_ptt_observable(&port, rig.model));
    for value in [f32::NAN, f32::INFINITY] {
        assert_eq!(normalized_level(value), Err(Error::InvalidState));
    }
    assert_eq!(normalized_level(0.5), Ok(500_000));
    for invalid in [0, 3] {
        assert_eq!(antenna_number(invalid), Err(Error::InvalidState));
    }
    assert_eq!(antenna_number(1), Ok(1));
    assert_eq!(antenna_number(1 << 30), Ok(31));
}

#[test]
fn failed_native_operating_calls_keep_the_backend_error() {
    unsafe extern "C" fn mw(_: *mut raw::RIG, _: *mut f32, _: u32, _: f64, _: u64) -> i32 {
        -77
    }
    unsafe extern "C" fn power(_: *mut raw::RIG, _: *mut u32, _: f32, _: f64, _: u64) -> i32 {
        -77
    }
    unsafe extern "C" fn shift(_: *mut raw::RIG, _: u32, _: raw::rptr_shift_t) -> i32 {
        -77
    }
    unsafe extern "C" fn rit(_: *mut raw::RIG, _: u32, _: *mut raw::shortfreq_t) -> i32 {
        -77
    }
    unsafe extern "C" fn tone(_: *mut raw::RIG, _: u32, _: *mut u32) -> i32 {
        -77
    }
    let mut rig = dummy();
    rig.set_frequency(146_520_000).unwrap();
    rig.set_mode_width(1, 15000).unwrap();
    // SAFETY: each initialized rig owns this copied capability record; replacing
    // its callbacks cannot alter another test/radio or access real hardware.
    unsafe {
        let caps = (*rig.pointer().unwrap()).caps;
        (*caps).mW2power = Some(mw);
        (*caps).power2mW = Some(power);
        (*caps).set_rptr_shift = Some(shift);
        (*caps).get_rit = Some(rit);
        (*caps).get_ctcss_tone = Some(tone);
    }
    assert_eq!(rig.set(Setting::Power, 1000), Err(Error::Backend(-77)));
    assert_eq!(rig.get(Setting::Power), Err(Error::Backend(-77)));
    assert_eq!(rig.set(Setting::Offset, 0), Err(Error::Backend(-77)));
    assert_eq!(rig.get(Setting::Rit), Err(Error::Backend(-77)));
    assert_eq!(rig.get(Setting::TransmitCtcss), Err(Error::Backend(-77)));
}
