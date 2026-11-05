//! Exact C records; native handles and native enum layouts stay private.

use crate::{Error, Rig, Setting, model_names};
use std::{ffi::c_void, mem, ptr, slice, str};

/// Borrowed UTF-8 span; output spans reference immutable static strings.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StringSpan {
    data: *const u8,
    size: usize,
}

impl StringSpan {
    fn from_static(value: &'static str) -> Self {
        Self {
            data: value.as_ptr(),
            size: value.len(),
        }
    }
    unsafe fn input<'a>(self) -> Result<&'a str, Error> {
        if self.size == 0 {
            return Ok("");
        }
        if self.size > isize::MAX as usize || self.data.is_null() {
            return Err(Error::InvalidState);
        }
        // SAFETY: the C caller lends a readable span for this call.
        str::from_utf8(unsafe { slice::from_raw_parts(self.data, self.size) })
            .map_err(|_| Error::InvalidOption)
    }
}

/// One copied named transport option.
#[repr(C)]
pub struct OptionRecord {
    name: StringSpan,
    value: StringSpan,
}

/// Full setup contract, using no native library types.
#[repr(C)]
pub struct Config {
    struct_size: usize,
    abi_version: u32,
    reserved: u32,
    model: StringSpan,
    options: *const OptionRecord,
    option_count: usize,
    timeout_ms: u32,
    retries: u32,
    timeout_retries: u32,
    reserved2: u32,
}

/// Copied scalar capability.
#[repr(C)]
pub struct SettingRecord {
    struct_size: usize,
    selector: u32,
    access: u32,
    name: StringSpan,
    value_kind: u32,
    units: u32,
    decimal_places: u32,
    reserved: u32,
    minimum: i64,
    maximum: i64,
    choice_count: usize,
}

/// Actual per-connection hardware coverage, with shared mode bits.
#[repr(C)]
pub struct RangeRecord {
    struct_size: usize,
    minimum_hz: u64,
    maximum_hz: u64,
    modes: u64,
    minimum_power_mw: i32,
    maximum_power_mw: i32,
}

/// Dynamically linked dependency contract. Function pointers use only C ABI records.
#[repr(C)]
pub struct Api {
    struct_size: usize,
    abi_version: u32,
    reserved: u32,
    name: *const u8,
    model_count: extern "C" fn() -> usize,
    model_name: unsafe extern "C" fn(usize, *mut StringSpan) -> i32,
    open: unsafe extern "C" fn(*const Config, *mut *mut c_void) -> i32,
    close: unsafe extern "C" fn(*mut c_void) -> i32,
    destroy: unsafe extern "C" fn(*mut c_void),
    setting: unsafe extern "C" fn(*mut c_void, usize, *mut SettingRecord) -> i32,
    choice: unsafe extern "C" fn(*mut c_void, u32, usize, *mut i64) -> i32,
    get: unsafe extern "C" fn(*mut c_void, u32, *mut i64) -> i32,
    set: unsafe extern "C" fn(*mut c_void, u32, i64) -> i32,
    transport_budget_ms: unsafe extern "C" fn(*mut c_void, *mut u64) -> i32,
    range: unsafe extern "C" fn(*mut c_void, u32, usize, *mut RangeRecord) -> i32,
    set_mode_width: unsafe extern "C" fn(*mut c_void, i64, u32) -> i32,
    set_split_vfos: unsafe extern "C" fn(*mut c_void, i64, u32, i64) -> i32,
    set_power_mw: unsafe extern "C" fn(*mut c_void, u32, u64, i64) -> i32,
    set_split_mode_width: unsafe extern "C" fn(*mut c_void, i64, u32) -> i32,
    get_power_mw: unsafe extern "C" fn(*mut c_void, u64, i64, *mut u32) -> i32,
}

// SAFETY: descriptor and pointed-to name are immutable for the DSO lifetime.
unsafe impl Sync for Api {}

fn code(result: Result<(), Error>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(Error::UnknownModel) => -1001,
        Err(Error::InvalidOption) => -1002,
        Err(Error::InvalidState) => -1003,
        Err(Error::WrongThread) => -1004,
        Err(Error::Backend(code)) => code,
    }
}

fn aligned<T>(pointer: *const T) -> bool {
    !pointer.is_null() && pointer.is_aligned()
}

unsafe fn owner<'a>(context: *mut c_void) -> Result<&'a mut Rig, Error> {
    if !aligned(context.cast::<Rig>()) {
        return Err(Error::InvalidState);
    }
    // SAFETY: caller exclusively lends the live opaque handle returned by open.
    let rig = unsafe { &mut *context.cast::<Rig>() };
    rig.pointer()?;
    Ok(rig)
}

extern "C" fn model_count() -> usize {
    model_names().len()
}

unsafe extern "C" fn model_name(index: usize, out: *mut StringSpan) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    let names = model_names();
    let Some(name) = names.get(index) else {
        return 1;
    };
    // SAFETY: caller supplies a complete aligned output record.
    unsafe { out.write(StringSpan::from_static(name)) };
    0
}

unsafe extern "C" fn open(config: *const Config, out: *mut *mut c_void) -> i32 {
    if !aligned(config) || !aligned(out) {
        return -1003;
    }
    unsafe { out.write(ptr::null_mut()) };
    // Read only the size prefix until the caller's complete record is admitted.
    if unsafe { config.cast::<usize>().read() } != mem::size_of::<Config>() {
        return -1003;
    }
    // SAFETY: the exact complete record size was admitted above.
    let config = unsafe { &*config };
    if config.abi_version != 1 || config.reserved != 0 || config.reserved2 != 0 {
        return -1003;
    }
    code((|| {
        if config.option_count > isize::MAX as usize / mem::size_of::<OptionRecord>()
            || (config.option_count != 0 && !aligned(config.options))
        {
            return Err(Error::InvalidState);
        }
        let records = if config.option_count == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(config.options, config.option_count) }
        };
        let mut options = Vec::with_capacity(records.len() + 3);
        for item in records {
            let name = unsafe { item.name.input()? };
            if matches!(name, "timeout" | "retry" | "timeout_retry") {
                return Err(Error::InvalidOption);
            }
            options.push((name, unsafe { item.value.input()? }));
        }
        let timeout = config.timeout_ms.to_string();
        let retries = config.retries.to_string();
        let read_retries = config.timeout_retries.to_string();
        options.extend([
            ("timeout", timeout.as_str()),
            ("retry", retries.as_str()),
            ("timeout_retry", read_retries.as_str()),
        ]);
        let rig = Rig::open(unsafe { config.model.input()? }, &options)?;
        unsafe { out.write(Box::into_raw(Box::new(rig)).cast()) };
        Ok(())
    })())
}

unsafe extern "C" fn close(context: *mut c_void) -> i32 {
    code(unsafe { owner(context) }.and_then(Rig::close))
}

unsafe extern "C" fn destroy(context: *mut c_void) {
    if !aligned(context.cast::<Rig>()) {
        return;
    }
    // SAFETY: caller returns the opaque allocation once, on its creating thread.
    unsafe { drop(Box::from_raw(context.cast::<Rig>())) };
}

unsafe extern "C" fn setting(context: *mut c_void, index: usize, out: *mut SettingRecord) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    if unsafe { (*out).struct_size } != mem::size_of::<SettingRecord>() {
        return -1003;
    }
    let catalog = match unsafe { owner(context) }.and_then(|rig| rig.catalog()) {
        Ok(value) => value,
        Err(error) => return code(Err(error)),
    };
    let Some(entry) = catalog.get(index) else {
        return 1;
    };
    unsafe {
        out.write(SettingRecord {
            struct_size: mem::size_of::<SettingRecord>(),
            selector: entry.setting as u32,
            access: u32::from(entry.readable) | (u32::from(entry.writable) << 1),
            name: StringSpan::from_static(entry.name),
            value_kind: entry.value_kind as u32,
            units: entry.units as u32,
            decimal_places: entry.decimal_places,
            reserved: 0,
            minimum: entry.minimum,
            maximum: entry.maximum,
            choice_count: entry.choices.len(),
        })
    };
    0
}

unsafe extern "C" fn choice(
    context: *mut c_void,
    selector: u32,
    index: usize,
    out: *mut i64,
) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    let selected = match Setting::from_selector(selector) {
        Ok(value) => value,
        Err(error) => return code(Err(error)),
    };
    let catalog = match unsafe { owner(context) }.and_then(|rig| rig.catalog()) {
        Ok(value) => value,
        Err(error) => return code(Err(error)),
    };
    let Some(entry) = catalog.iter().find(|item| item.setting == selected) else {
        return -1003;
    };
    let Some(value) = entry.choices.get(index) else {
        return 1;
    };
    unsafe { out.write(*value) };
    0
}

unsafe extern "C" fn get(context: *mut c_void, selector: u32, out: *mut i64) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    code((|| {
        let value = unsafe { owner(context)? }.get(Setting::from_selector(selector)?)?;
        unsafe { out.write(value) };
        Ok(())
    })())
}

unsafe extern "C" fn set(context: *mut c_void, selector: u32, value: i64) -> i32 {
    code((|| {
        unsafe { owner(context)? }.set(Setting::from_selector(selector)?, value)
    })())
}

unsafe extern "C" fn transport_budget_ms(context: *mut c_void, out: *mut u64) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    code((|| {
        unsafe { out.write(owner(context)?.transport_budget_ms()) };
        Ok(())
    })())
}
unsafe extern "C" fn set_mode_width(context: *mut c_void, mode: i64, width: u32) -> i32 {
    code((|| unsafe { owner(context)? }.set_mode_width(mode, width))())
}
unsafe extern "C" fn set_split_vfos(
    context: *mut c_void,
    receive: i64,
    enabled: u32,
    transmit: i64,
) -> i32 {
    if enabled > 1 {
        return -1003;
    }
    code((|| {
        unsafe { owner(context)? }.set_split_vfos(receive, enabled != 0, transmit)
    })())
}
unsafe extern "C" fn set_split_mode_width(context: *mut c_void, mode: i64, width: u32) -> i32 {
    code((|| {
        unsafe { owner(context)? }.set_split_mode_width(mode, width)
    })())
}
unsafe extern "C" fn get_power_mw(
    context: *mut c_void,
    transmit_hz: u64,
    mode: i64,
    out: *mut u32,
) -> i32 {
    if !aligned(out) {
        return -1003;
    }
    code((|| {
        let value = unsafe { owner(context)? }.get_power_mw(transmit_hz, mode)?;
        // SAFETY: the caller supplies one aligned writable scalar.
        unsafe { out.write(value) };
        Ok(())
    })())
}
unsafe extern "C" fn set_power_mw(
    context: *mut c_void,
    power: u32,
    transmit_hz: u64,
    mode: i64,
) -> i32 {
    code((|| {
        unsafe { owner(context)? }.set_power_mw(power, transmit_hz, mode)
    })())
}

unsafe extern "C" fn range(
    context: *mut c_void,
    transmit: u32,
    index: usize,
    out: *mut RangeRecord,
) -> i32 {
    if transmit > 1 || !aligned(out) {
        return -1003;
    }
    if unsafe { (*out).struct_size } != mem::size_of::<RangeRecord>() {
        return -1003;
    }
    let ranges = match unsafe { owner(context) }.and_then(|rig| rig.coverage(transmit != 0)) {
        Ok(value) => value,
        Err(error) => return code(Err(error)),
    };
    let Some(value) = ranges.get(index) else {
        return 1;
    };
    unsafe {
        out.write(RangeRecord {
            struct_size: mem::size_of::<RangeRecord>(),
            minimum_hz: value.minimum_hz,
            maximum_hz: value.maximum_hz,
            modes: value.modes,
            minimum_power_mw: value.minimum_power_mw,
            maximum_power_mw: value.maximum_power_mw,
        })
    };
    0
}

static API: Api = Api {
    struct_size: mem::size_of::<Api>(),
    abi_version: 1,
    reserved: 0,
    name: c"rptadv.hamlib".as_ptr().cast(),
    model_count,
    model_name,
    open,
    close,
    destroy,
    setting,
    choice,
    get,
    set,
    transport_budget_ms,
    range,
    set_mode_width,
    set_split_vfos,
    set_power_mw,
    set_split_mode_width,
    get_power_mw,
};

/// Borrow the immutable ABI-1 descriptor while this shared object remains loaded.
#[unsafe(no_mangle)]
pub extern "C" fn rptadv_hamlib_api_v1() -> *const Api {
    &API
}

#[cfg(test)]
#[cfg_attr(coverage, coverage(off))]
mod tests;
