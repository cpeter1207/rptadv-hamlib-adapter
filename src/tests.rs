use super::*;

#[test]
fn hamlib_offset_capability_requires_shift_and_magnitude_operations() {
    let capability = |missing| {
        crate::catalog::operation_capabilities(
            Setting::Offset,
            raw::RIG_FUNCTION_GET_RPTR_OFFS,
            raw::RIG_FUNCTION_SET_RPTR_OFFS,
            |operation| operation != missing,
        )
    };
    let complete = capability(u32::MAX);
    assert_eq!(complete, (true, true));
    assert_eq!(capability(raw::RIG_FUNCTION_GET_RPTR_SHIFT), (false, true));
    assert_eq!(capability(raw::RIG_FUNCTION_SET_RPTR_SHIFT), (true, false));
    assert_eq!(
        crate::catalog::operation_capabilities(
            Setting::CarrierDetect,
            raw::RIG_FUNCTION_GET_DCD,
            u32::MAX,
            |_| true
        ),
        (true, false)
    );
}

#[test]
fn hamlib_catalog_declares_value_types_and_units() {
    use crate::catalog::{ValueKind as K, ValueUnit as U};
    let rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    let entries = rig.catalog().unwrap();
    for (setting, kind, units, places) in [
        (Setting::ReceiveFrequency, K::Unsigned, U::Hertz, 0),
        (Setting::Offset, K::Signed, U::Hertz, 0),
        (Setting::Mode, K::Enumeration, U::None, 0),
        (Setting::Split, K::Boolean, U::None, 0),
        (Setting::TransmitCtcss, K::Decimal, U::Hertz, 1),
        (Setting::TransmitDcs, K::Unsigned, U::DcsCode, 0),
        (Setting::Power, K::Unsigned, U::Milliwatts, 0),
        (Setting::AfGain, K::Decimal, U::Ratio, 6),
        (Setting::Preamp, K::Unsigned, U::Decibels, 0),
        (Setting::KeyerSpeed, K::Unsigned, U::WordsPerMinute, 0),
    ] {
        let item = entries
            .iter()
            .find(|entry| entry.setting == setting)
            .unwrap();
        assert_eq!(
            (item.value_kind, item.units, item.decimal_places),
            (kind, units, places)
        );
    }
}

#[test]
fn hamlib_dummy_supports_typed_roundtrip() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    rig.set_frequency(146_520_000).unwrap();
    assert_eq!(rig.frequency().unwrap(), 146_520_000);
    rig.set_ptt(true).unwrap();
    assert!(rig.ptt().unwrap());
    rig.set_ptt(false).unwrap();
    assert!(!rig.ptt().unwrap());
}

#[test]
fn hamlib_models_use_enumerated_names() {
    assert!(model_names().contains(&"RIG_MODEL_DUMMY"));
    assert!(model_names().contains(&"RIG_MODEL_NETRIGCTL"));
    assert!(model_names().contains(&"RIG_MODEL_IC7300"));
    assert!(model_names().contains(&"RIG_MODEL_IC9700"));
    assert!(Rig::open("1", &[]).is_err());
    assert!(Rig::open("RIG_MODEL_NOT_A_RIG", &[]).is_err());
}

#[test]
fn hamlib_dummy_roundtrips_operating_catalog() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    for (setting, value) in [
        (Setting::ReceiveFrequency, 146_520_000),
        (Setting::Mode, 3),
        (Setting::Bandwidth, 2_400),
        (Setting::Rit, -100),
        (Setting::Xit, 100),
        (Setting::Offset, -600_000),
        (Setting::TransmitCtcss, 1_148),
        (Setting::ReceiveCtcss, 1_148),
        (Setting::TransmitDcs, 0o23),
        (Setting::ReceiveDcs, 0o754),
        (Setting::AfGain, 500_000),
    ] {
        let capability = rig
            .catalog()
            .unwrap()
            .into_iter()
            .find(|entry| entry.setting == setting)
            .unwrap();
        assert!(capability.readable && capability.writable, "{setting:?}");
        rig.set(setting, value).unwrap();
        assert_eq!(rig.get(setting).unwrap(), value, "{setting:?}");
    }
    assert_eq!(rig.get(Setting::Mode).unwrap(), 3);
    assert_eq!(rig.get(Setting::Bandwidth).unwrap(), 2_400);
}

#[test]
fn hamlib_complete_writes_take_their_companion_state_from_the_caller() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    rig.set_frequency(146_520_000).unwrap();
    rig.set_mode_width(3, 2400).unwrap();
    assert_eq!(rig.get(Setting::Mode).unwrap(), 3);
    assert_eq!(rig.get(Setting::Bandwidth).unwrap(), 2400);
    rig.set_split_vfos(1, true, 2).unwrap();
    assert_eq!(rig.get(Setting::Split).unwrap(), 1);
    assert_eq!(rig.get(Setting::TransmitVfo).unwrap(), 2);
    rig.set_power_mw(5000, 146_520_000, 1).unwrap();
    assert!(rig.set_mode_width(99, 2400).is_err());
    assert!(rig.set_split_vfos(99, true, 2).is_err());
    assert!(rig.set_power_mw(5000, 0, 1).is_err());
}

#[test]
fn hamlib_split_mode_and_power_use_the_transmit_context() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    rig.set_frequency(146520000).unwrap();
    rig.set_split_vfos(1, true, 2).unwrap();
    rig.set(Setting::TransmitFrequency, 435000000).unwrap();
    rig.set_mode_width(1, 15000).unwrap();
    rig.set_split_mode_width(3, 2400).unwrap();
    assert_eq!(rig.get(Setting::SplitMode), Ok(3));
    assert_eq!(rig.get(Setting::SplitBandwidth), Ok(2400));
    rig.set_power_mw(5000, 435000000, 3).unwrap();
    assert!(rig.get_power_mw(435000000, 3).unwrap() > 0);
    assert!(rig.get_power_mw(0, 3).is_err());
    assert!(rig.get_power_mw(435000000, 99).is_err());
    assert!(rig.set_split_mode_width(99, 2400).is_err());
}

#[test]
fn hamlib_rejects_unsupported_capability_before_mutation() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    rig.set_frequency(146_520_000).unwrap();
    assert!(rig.set(Setting::ReceiveFrequency, -1).is_err());
    assert!(rig.set(Setting::TransmitDcs, 512).is_err());
    assert!(rig.set(Setting::AfGain, 1_000_001).is_err());
    assert_eq!(rig.frequency().unwrap(), 146_520_000);
}

#[test]
fn hamlib_dcs_uses_actual_nine_bit_codes() {
    assert_eq!(dcs_to_native(0o23).unwrap(), 23);
    assert_eq!(dcs_to_native(0o754).unwrap(), 754);
    assert_eq!(dcs_from_native(23).unwrap(), 0o23);
    assert_eq!(dcs_from_native(754).unwrap(), 0o754);
    assert!(dcs_from_native(789).is_err());
    assert!(dcs_to_native(512).is_err());
}

#[test]
fn hamlib_transport_budget_is_finite_and_configurable() {
    let rig = Rig::open("RIG_MODEL_DUMMY", &[("timeout", "100"), ("retry", "1")]).unwrap();
    assert_eq!(rig.transport_budget_ms(), 200);
    assert!(Rig::open("RIG_MODEL_DUMMY", &[("timeout", "0")]).is_err());
    assert!(Rig::open("RIG_MODEL_DUMMY", &[("retry", "-1")]).is_err());
    assert!(Rig::open("RIG_MODEL_DUMMY", &[("timeout", "secret-invalid-value")]).is_err());
}

#[test]
fn hamlib_independent_rigs_do_not_share_operating_state() {
    let workers: Vec<_> = [145_000_000, 146_000_000]
        .into_iter()
        .map(|frequency| {
            std::thread::spawn(move || {
                let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
                for _ in 0..25 {
                    rig.set_frequency(frequency).unwrap();
                    assert_eq!(rig.frequency().unwrap(), frequency);
                }
            })
        })
        .collect();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn hamlib_owner_check_rejects_a_different_native_thread() {
    let rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    let borrowed = std::ptr::from_ref(&rig) as usize;
    let result = std::thread::spawn(move || {
        // SAFETY: the original owner waits for this joined diagnostic and never
        // mutates the handle. The rejected call reads only immutable owner identity.
        unsafe { (*(borrowed as *const Rig)).pointer() }.map(|_| ())
    })
    .join()
    .unwrap();
    assert_eq!(result, Err(Error::WrongThread));
}

#[test]
fn hamlib_catalog_uses_native_discrete_and_fractional_constraints() {
    let mut rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    let catalog = rig.catalog().unwrap();
    let tone = catalog
        .iter()
        .find(|entry| entry.setting == Setting::TransmitCtcss)
        .unwrap();
    assert_eq!(tone.decimal_places, 1);
    // Dummy leaves its tone table unspecified; do not invent a restricted list.
    assert!(tone.choices.is_empty());
    assert_eq!(
        crate::catalog::tone_choices([1148, 1188, 0].as_ptr(), false).unwrap(),
        vec![0, 1148, 1188]
    );
    assert_eq!(
        crate::catalog::tone_choices([23, 754, 0].as_ptr(), true).unwrap(),
        vec![0, 0o23, 0o754]
    );
    rig.set(Setting::ReceiveVfo, 0).unwrap();
    for setting in [
        Setting::Preamp,
        Setting::Attenuator,
        Setting::Agc,
        Setting::NoiseBlanker,
    ] {
        let entry = catalog
            .iter()
            .find(|entry| entry.setting == setting)
            .unwrap();
        let value = entry
            .choices
            .iter()
            .copied()
            .find(|value| *value != 0)
            .unwrap_or(1);
        rig.set(setting, value).unwrap();
        assert_eq!(rig.get(setting).unwrap(), value);
    }
}

#[test]
fn hamlib_runtime_coverage_is_owned_per_connection() {
    let rig = Rig::open("RIG_MODEL_DUMMY", &[]).unwrap();
    let receive = rig.coverage(false).unwrap();
    assert!(receive.iter().any(|range| range.minimum_hz <= 146_520_000
        && range.maximum_hz >= 146_520_000
        && range.modes & 2 != 0));
    assert!(!rig.coverage(true).unwrap().is_empty());
    let declared = rig
        .catalog()
        .unwrap()
        .into_iter()
        .find(|entry| entry.setting == Setting::Mode)
        .unwrap()
        .choices
        .into_iter()
        .fold(0_u64, |bits, selector| bits | (1_u64 << selector));
    let covered = receive.iter().fold(0_u64, |bits, range| bits | range.modes);
    assert_eq!(
        covered, declared,
        "shared masks shift by the mode selector, reserving bit zero"
    );
}

#[test]
fn hamlib_transport_error_preserves_first_cause() {
    use std::{
        io::Read,
        net::{Shutdown, TcpListener},
        time::Duration,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    listener.set_nonblocking(true).unwrap();
    let (finished, client_finished) = std::sync::mpsc::sync_channel(1);
    let server = std::thread::spawn(move || {
        loop {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(1)))
                        .unwrap();
                    let mut request = [0; 128];
                    let _ = stream.read(&mut request);
                    stream.shutdown(Shutdown::Both).unwrap();
                    return;
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    // Other fixtures may own Hamlib's frontend lifecycle lock.
                    // Wait for this client's result, not time spent before open.
                    assert!(
                        matches!(
                            client_finished.try_recv(),
                            Err(std::sync::mpsc::TryRecvError::Empty)
                        ),
                        "loopback client finished without connecting"
                    );
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("loopback fixture did not connect: {error}"),
            }
        }
    });
    let result = Rig::open(
        "RIG_MODEL_NETRIGCTL",
        &[
            ("rig_pathname", &address),
            ("timeout", "50"),
            ("retry", "0"),
            ("timeout_retry", "0"),
        ],
    );
    let _ = finished.send(());
    assert!(matches!(result, Err(Error::Backend(code)) if code < 0));
    server.join().unwrap();
    // Failed setup/cleanup must neither overwrite the cause nor poison the next owner.
    assert!(Rig::open("RIG_MODEL_DUMMY", &[]).is_ok());
}
