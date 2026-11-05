//! Owner admission, exact readings and cleanup retain their public failures.

use super::*;

#[test]
fn frequency_and_transport_boundaries_reject_inexact_or_unbounded_values() {
    for value in [
        f64::NAN,
        f64::INFINITY,
        0.0,
        -1.0,
        (1_u64 << 53) as f64 + 2.0,
        1.5,
    ] {
        assert_eq!(integer_frequency(value), Err(Error::InvalidState));
    }
    for value in [1, 146_520_000, 1_u64 << 53] {
        assert_eq!(integer_frequency(value as f64), Ok(value));
    }
    for (name, value) in [
        ("timeout", "10001"),
        ("retry", "11"),
        ("timeout_retry", "101"),
        ("timeout_retry", "invalid"),
    ] {
        assert!(matches!(
            Rig::open("RIG_MODEL_DUMMY", &[(name, value)]),
            Err(Error::InvalidOption)
        ));
    }
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    for value in [0, (1_u64 << 53) + 1] {
        assert_eq!(rig.set_frequency(value), Err(Error::InvalidState));
    }
    for (name, value) in [
        ("unknown-option", "1"),
        ("bad\0name", "1"),
        ("ptt_type", "bad\0value"),
    ] {
        assert_eq!(rig.option(name, value), Err(Error::InvalidOption));
    }
    assert_eq!(Error::Backend(-7).to_string(), "Backend(-7)");
    assert_eq!(status(-7), Err(Error::Backend(-7)));
    rig.close().unwrap();
    rig.close().unwrap();
}

#[test]
fn negative_native_cache_settings_preserve_the_original_failure() {
    assert!(matches!(
        Rig::open("RIG_MODEL_DUMMY", &[("cache_timeout", "-7")]),
        Err(Error::Backend(-7))
    ));
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    rig.option("cache_timeout", "-9").unwrap();
    assert_eq!(rig.frequency(), Err(Error::Backend(-9)));
    rig.option("cache_timeout", "173").unwrap();
    assert_eq!(rig.fresh_read(|_| Ok(42)), Ok(42));
}

#[test]
fn physical_ptt_observations_reject_unknown_native_values() {
    assert_eq!(observed_ptt(raw::RIG_PTT_OFF), Ok(false));
    for value in [raw::RIG_PTT_ON, raw::RIG_PTT_ON_MIC, raw::RIG_PTT_ON_DATA] {
        assert_eq!(observed_ptt(value), Ok(true));
    }
    assert_eq!(observed_ptt(u32::MAX), Err(Error::InvalidState));
}
