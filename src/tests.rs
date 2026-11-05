use super::*;

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
        && range.modes & 1 != 0));
    assert!(!rig.coverage(true).unwrap().is_empty());
}

#[test]
fn hamlib_transport_error_preserves_first_cause() {
    use std::{
        io::Read,
        net::{Shutdown, TcpListener},
        time::{Duration, Instant},
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap().to_string();
    listener.set_nonblocking(true).unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(3);
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
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(1))
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
    assert!(matches!(result, Err(Error::Backend(code)) if code < 0));
    server.join().unwrap();
    // Failed setup/cleanup must neither overwrite the cause nor poison the next owner.
    assert!(Rig::open("RIG_MODEL_DUMMY", &[]).is_ok());
}
