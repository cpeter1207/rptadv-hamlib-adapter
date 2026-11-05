//! Exercise the public C boundary with the installed library's dummy radio.

use super::*;

fn span(value: &str) -> StringSpan {
    StringSpan {
        data: value.as_ptr(),
        size: value.len(),
    }
}

fn config() -> Config {
    Config {
        struct_size: mem::size_of::<Config>(),
        abi_version: 1,
        reserved: 0,
        model: span("RIG_MODEL_DUMMY"),
        options: ptr::null(),
        option_count: 0,
        timeout_ms: 100,
        retries: 1,
        timeout_retries: 0,
        reserved2: 0,
    }
}

fn bad<T>() -> *mut T {
    ptr::without_provenance_mut(1)
}

#[test]
fn c_admission_rejects_invalid_records_before_reading_them() {
    // SAFETY: rejected pointers are null/misaligned; other records live through each call.
    unsafe {
        let api = &*rptadv_hamlib_api_v1();
        assert_eq!(api.struct_size, mem::size_of::<Api>());
        assert_eq!((api.abi_version, api.reserved), (1, 0));
        let mut output = ptr::null_mut();
        for pointer in [ptr::null(), bad::<Config>().cast_const()] {
            assert_eq!((api.open)(pointer, &mut output), -1003);
        }
        for pointer in [ptr::null_mut(), bad::<*mut c_void>()] {
            assert_eq!((api.open)(&config(), pointer), -1003);
        }
        for field in 0..4 {
            let mut value = config();
            match field {
                0 => value.struct_size -= 1,
                1 => value.abi_version = 2,
                2 => value.reserved = 1,
                _ => value.reserved2 = 1,
            }
            assert_eq!((api.open)(&value, &mut output), -1003);
            assert!(output.is_null());
        }
        for (pointer, count) in [
            (ptr::null(), 1),
            (bad::<OptionRecord>().cast_const(), 1),
            (ptr::null(), usize::MAX),
        ] {
            let mut value = config();
            value.options = pointer;
            value.option_count = count;
            assert_eq!((api.open)(&value, &mut output), -1003);
        }
        for name in ["timeout", "retry", "timeout_retry"] {
            let record = OptionRecord {
                name: span(name),
                value: span("1"),
            };
            let mut value = config();
            value.options = &record;
            value.option_count = 1;
            assert_eq!((api.open)(&value, &mut output), -1002);
        }
        let invalid_utf8 = [255];
        for (input, expected) in [
            (span(""), -1001),
            (span("RIG_MODEL_MISSING"), -1001),
            (
                StringSpan {
                    data: ptr::null(),
                    size: 1,
                },
                -1003,
            ),
            (
                StringSpan {
                    data: ptr::null(),
                    size: usize::MAX,
                },
                -1003,
            ),
            (
                StringSpan {
                    data: invalid_utf8.as_ptr(),
                    size: 1,
                },
                -1002,
            ),
        ] {
            let mut value = config();
            value.model = input;
            assert_eq!((api.open)(&value, &mut output), expected);
        }
        assert_eq!(code(Err(Error::Backend(-91))), -91);
        let mut model = span("");
        assert!((api.model_count)() > 100);
        assert_eq!((api.model_name)(0, &mut model), 0);
        assert!(!model.input().unwrap().is_empty());
        assert_eq!((api.model_name)((api.model_count)(), &mut model), 1);
        assert_eq!((api.model_name)(0, ptr::null_mut()), -1003);
        assert_eq!((api.model_name)(0, bad()), -1003);
        for context in [ptr::null_mut(), bad::<Rig>().cast()] {
            let mut value = 0;
            let mut budget = 0;
            let mut power = 0;
            let mut entry: SettingRecord = mem::zeroed();
            entry.struct_size = mem::size_of::<SettingRecord>();
            let mut range: RangeRecord = mem::zeroed();
            range.struct_size = mem::size_of::<RangeRecord>();
            assert_eq!((api.close)(context), -1003);
            assert_eq!((api.setting)(context, 0, &mut entry), -1003);
            assert_eq!((api.choice)(context, 3, 0, &mut value), -1003);
            assert_eq!((api.get)(context, 1, &mut value), -1003);
            assert_eq!((api.set)(context, 1, 146_520_000), -1003);
            assert_eq!((api.transport_budget_ms)(context, &mut budget), -1003);
            assert_eq!((api.range)(context, 0, 0, &mut range), -1003);
            assert_eq!((api.set_mode_width)(context, 3, 2400), -1003);
            assert_eq!((api.set_split_vfos)(context, 1, 1, 2), -1003);
            assert_eq!((api.set_power_mw)(context, 1000, 146_520_000, 1), -1003);
            assert_eq!((api.set_split_mode_width)(context, 3, 2400), -1003);
            assert_eq!(
                (api.get_power_mw)(context, 146_520_000, 1, &mut power),
                -1003
            );
            (api.destroy)(context);
        }
    }
}

#[test]
fn c_dummy_owner_copies_capabilities_and_operates_through_every_entry() {
    // SAFETY: the creating thread owns the handle; all spans and outputs are live.
    unsafe {
        let api = &*rptadv_hamlib_api_v1();
        let record = OptionRecord {
            name: span("ptt_type"),
            value: span("RIG"),
        };
        let mut cfg = config();
        cfg.options = &record;
        cfg.option_count = 1;
        let mut context = ptr::null_mut();
        assert_eq!((api.open)(&cfg, &mut context), 0);
        assert!(!context.is_null());
        let mut value = 0;
        let mut budget = 0;
        let mut power = 0;
        assert_eq!((api.transport_budget_ms)(context, &mut budget), 0);
        assert_eq!(budget, 200);
        assert_eq!((api.set)(context, 1, 146_520_000), 0);
        assert_eq!((api.get)(context, 1, &mut value), 0);
        assert_eq!(value, 146_520_000);
        assert_eq!((api.set)(context, u32::MAX, 1), -1003);
        assert_eq!((api.get)(context, u32::MAX, &mut value), -1003);
        assert_eq!((api.set_mode_width)(context, 3, 2400), 0);
        assert_eq!((api.set_split_vfos)(context, 1, 1, 2), 0);
        assert_eq!((api.set_split_vfos)(context, 1, 2, 2), -1003);
        assert_eq!((api.set_power_mw)(context, 5000, 146_520_000, 1), 0);
        assert_eq!((api.set_split_mode_width)(context, 3, 2400), 0);
        assert_eq!((api.get_power_mw)(context, 146_520_000, 3, &mut power), 0);
        assert!(power > 0);
        let mut entry: SettingRecord = mem::zeroed();
        assert_eq!((api.setting)(context, 0, &mut entry), -1003);
        entry.struct_size = mem::size_of::<SettingRecord>();
        assert_eq!((api.setting)(context, usize::MAX, &mut entry), 1);
        let mut found_choices = false;
        for index in 0..100 {
            let status = (api.setting)(context, index, &mut entry);
            if status == 1 {
                break;
            }
            assert_eq!(status, 0);
            assert!(!entry.name.input().unwrap().is_empty());
            assert_eq!(entry.reserved, 0);
            assert_eq!(
                (api.choice)(context, entry.selector, entry.choice_count, &mut value),
                1
            );
            if entry.choice_count > 0 {
                assert_eq!((api.choice)(context, entry.selector, 0, &mut value), 0);
                found_choices = true;
            }
        }
        assert!(found_choices);
        assert_eq!((api.choice)(context, u32::MAX, 0, &mut value), -1003);
        // A radio may omit optional controls. Such selectors must not acquire
        // fabricated choices merely because the selector itself is recognized.
        let handle = (*context.cast::<Rig>()).pointer().unwrap();
        let state = &mut *crate::raw::rig_data_pointer(handle, crate::raw::RIG_PTRX_STATE)
            .cast::<crate::raw::rig_state>();
        state.has_get_func = 0;
        state.has_set_func = 0;
        assert_eq!(
            (api.choice)(context, Setting::Tuner as u32, 0, &mut value),
            -1003
        );
        let mut range: RangeRecord = mem::zeroed();
        assert_eq!((api.range)(context, 0, 0, &mut range), -1003);
        range.struct_size = mem::size_of::<RangeRecord>();
        for transmit in [0, 1] {
            assert_eq!((api.range)(context, transmit, 0, &mut range), 0);
            assert!(range.maximum_hz > range.minimum_hz);
            assert_eq!((api.range)(context, transmit, usize::MAX, &mut range), 1);
        }
        assert_eq!((api.range)(context, 2, 0, &mut range), -1003);
        assert_eq!((api.setting)(context, 0, ptr::null_mut()), -1003);
        assert_eq!((api.choice)(context, 3, 0, ptr::null_mut()), -1003);
        assert_eq!((api.get)(context, 1, ptr::null_mut()), -1003);
        assert_eq!((api.transport_budget_ms)(context, ptr::null_mut()), -1003);
        assert_eq!((api.get_power_mw)(context, 1, 1, ptr::null_mut()), -1003);
        assert_eq!((api.range)(context, 0, 0, ptr::null_mut()), -1003);
        let address = context as usize;
        assert_eq!(
            std::thread::spawn(move || (api.close)(address as *mut c_void))
                .join()
                .unwrap(),
            -1004
        );
        assert_eq!((api.close)(context), 0);
        assert_eq!((api.get)(context, 1, &mut value), -1003);
        (api.destroy)(context);
    }
}
