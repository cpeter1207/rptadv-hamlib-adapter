//! One thread-owned RIG handle, with only frontend lifecycle serialized globally.

use crate::raw;
use std::{
    ffi::CString,
    ptr::NonNull,
    sync::{Mutex, Once},
};

include!(concat!(env!("OUT_DIR"), "/models.rs"));

// Hamlib frontend registration and the opened-rig list are process-global.
// Only lifecycle operations use this lock; ordinary per-rig I/O and every
// audio callback remain independent. Lifecycle may include bounded open I/O.
static FRONTEND: Mutex<()> = Mutex::new(());
static DEBUG_INIT: Once = Once::new();

/// Adapter failure without a secret-bearing configuration string.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// Model is not one of the installed header's symbolic names.
    UnknownModel,
    /// Name/value cannot be represented by the C configuration contract.
    InvalidOption,
    /// Returned native data cannot be represented accurately.
    InvalidState,
    /// Another thread attempted to use this physical owner's handle.
    WrongThread,
    /// Original Hamlib result, including truncation or unavailable operations.
    Backend(i32),
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}
impl std::error::Error for Error {}

/// Enumerate installed symbolic names; numeric model values remain private.
pub fn model_names() -> Vec<&'static str> {
    MODELS
        .iter()
        .filter(|(_, value)| *value != 0)
        .map(|(name, _)| *name)
        .collect()
}

pub(crate) fn status(result: i32) -> Result<(), Error> {
    if result == 0 {
        Ok(())
    } else {
        Err(Error::Backend(result))
    }
}

/// Owned Hamlib rig, created, used and closed by one non-audio worker.
pub struct Rig {
    handle: Option<NonNull<raw::RIG>>,
    // Native identity avoids installing a Rust TLS destructor whose code might
    // be unloaded after the final rig closes but before this worker exits.
    owner: libc::pthread_t,
    opened: bool,
    owns_ptt: bool,
    pub(crate) model: u32,
    transport_budget_ms: u64,
}

impl Rig {
    /// Configure a named model using the library's supported transport options.
    pub fn open(model: &str, options: &[(&str, &str)]) -> Result<Self, Error> {
        let mut timeout = 500_u64;
        let mut retries = 2_u64;
        let mut read_retries = 0_u64;
        for (name, value) in options {
            match *name {
                "timeout" => timeout = value.parse().map_err(|_| Error::InvalidOption)?,
                "retry" => retries = value.parse().map_err(|_| Error::InvalidOption)?,
                "timeout_retry" => {
                    read_retries = value.parse().map_err(|_| Error::InvalidOption)?
                }
                _ => (),
            }
        }
        if !(1..=10_000).contains(&timeout) || retries > 10 || read_retries > 100 {
            return Err(Error::InvalidOption);
        }
        let id = MODELS
            .iter()
            .find(|(name, value)| *name == model && *value != 0)
            .map(|(_, value)| *value)
            .ok_or(Error::UnknownModel)?;
        let handle = {
            let _guard = FRONTEND
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // SAFETY: silence process-wide diagnostics before transport options can
            // contain credentials. The adapter returns original numeric errors.
            DEBUG_INIT.call_once(|| unsafe { raw::rig_set_debug(raw::RIG_DEBUG_NONE) });
            // SAFETY: frontend initialization is serialized; id comes from installed headers.
            NonNull::new(unsafe { raw::rig_init(id) }).ok_or(Error::UnknownModel)?
        };
        let mut rig = Self {
            handle: Some(handle),
            // SAFETY: pthread_self returns this live native owner's identity.
            owner: unsafe { libc::pthread_self() },
            opened: false,
            owns_ptt: false,
            model: id,
            transport_budget_ms: timeout * (retries + 1) * (read_retries + 1),
        };
        let timeout = timeout.to_string();
        let retries = retries.to_string();
        let read_retries = read_retries.to_string();
        let transport = [
            ("timeout", timeout.as_str()),
            ("retry", retries.as_str()),
            ("timeout_retry", read_retries.as_str()),
        ];
        for (name, value) in transport.into_iter().chain(
            options
                .iter()
                .copied()
                .filter(|(name, _)| !matches!(*name, "timeout" | "retry" | "timeout_retry")),
        ) {
            rig.option(name, value)?;
        }
        // This adapter's existing owner performs every poll. Hamlib otherwise
        // starts a background poller at open which also overwrites cache policy.
        // Disable its unsolicited/network workers before any can be started.
        // SAFETY: only this setup owner retains the unopened rig and its cache.
        let cache_timeout =
            unsafe { raw::rig_get_cache_timeout_ms(handle.as_ptr(), raw::HAMLIB_CACHE_ALL) };
        if cache_timeout < 0 {
            return Err(Error::Backend(cache_timeout));
        }
        for (name, value) in [
            ("poll_interval", "0"),
            ("async", "0"),
            ("multicast_data_addr", "0.0.0.0"),
            ("multicast_cmd_addr", "0.0.0.0"),
        ] {
            rig.option(name, value)?;
        }
        // SAFETY: setting poll_interval also changes this live rig's cache;
        // preserve the caller/default policy without creating another owner.
        status(unsafe {
            raw::rig_set_cache_timeout_ms(handle.as_ptr(), raw::HAMLIB_CACHE_ALL, cache_timeout)
        })?;
        // SAFETY: the current owner exclusively retains the configured handle.
        let result = {
            let _guard = FRONTEND
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            unsafe { raw::rig_open(handle.as_ptr()) }
        };
        if result != 0 {
            // SAFETY: close any partially opened transport, retaining the original error.
            let _guard = FRONTEND
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let _ = unsafe { raw::rig_close(handle.as_ptr()) };
            return Err(Error::Backend(result));
        }
        rig.opened = true;
        // Network open may import the server's timeout. Reapply the explicitly
        // configured transport policy to this handle after capability negotiation.
        for (name, value) in transport {
            rig.option(name, value)?;
        }
        Ok(rig)
    }

    /// Apply a frontend option on its sole owner before transport use.
    fn option(&self, name: &str, value: &str) -> Result<(), Error> {
        let handle = self.pointer()?;
        let name = CString::new(name).map_err(|_| Error::InvalidOption)?;
        let value = CString::new(value).map_err(|_| Error::InvalidOption)?;
        // SAFETY: the owner retains its rig and both NUL-terminated strings.
        let token = unsafe { raw::rig_token_lookup(handle, name.as_ptr()) };
        if token == 0 {
            return Err(Error::InvalidOption);
        }
        // SAFETY: the token belongs to this rig and value lives through return.
        status(unsafe { raw::rig_set_conf(handle, token, value.as_ptr()) })
    }

    /// Configured timeout times attempts for one transport exchange. A complete
    /// CAT operation may perform multiple exchanges; this is not a cancellation timer.
    pub fn transport_budget_ms(&self) -> u64 {
        self.transport_budget_ms
    }

    pub(crate) fn pointer(&self) -> Result<*mut raw::RIG, Error> {
        // SAFETY: the recorded owner is the still-retained creating pthread.
        if unsafe { libc::pthread_equal(libc::pthread_self(), self.owner) } == 0 {
            return Err(Error::WrongThread);
        }
        self.handle.map(NonNull::as_ptr).ok_or(Error::InvalidState)
    }

    /// Query the physical backend rather than Hamlib's recent setter cache.
    /// Cache policy belongs to this rig and is restored even on a read error.
    pub(crate) fn fresh_read<T>(
        &mut self,
        read: impl FnOnce(&mut Self) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let handle = self.pointer()?;
        // SAFETY: only this owner accesses the retained rig and its cache policy.
        let previous = unsafe { raw::rig_get_cache_timeout_ms(handle, raw::HAMLIB_CACHE_ALL) };
        if previous < 0 {
            return Err(Error::Backend(previous));
        }
        // SAFETY: zero disables this rig's frontend cache for the following read.
        status(unsafe { raw::rig_set_cache_timeout_ms(handle, raw::HAMLIB_CACHE_ALL, 0) })?;
        let result = read(self);
        // SAFETY: the read does not destroy the rig; restore its original policy.
        let restored = status(unsafe {
            raw::rig_set_cache_timeout_ms(handle, raw::HAMLIB_CACHE_ALL, previous)
        });
        result.and_then(|value| restored.map(|()| value))
    }

    /// Set integer-hertz frequency on the currently selected receive VFO.
    pub fn set_frequency(&mut self, hertz: u64) -> Result<(), Error> {
        if hertz == 0 || hertz > (1_u64 << 53) {
            return Err(Error::InvalidState);
        }
        let handle = self.pointer()?;
        // SAFETY: this worker is the sole API user; hertz is exactly representable.
        status(unsafe { raw::rig_set_freq(handle, raw::RPTADV_VFO_CURRENT, hertz as f64) })
    }

    /// Read integer-hertz receive frequency without silently truncating a native value.
    pub fn frequency(&mut self) -> Result<u64, Error> {
        self.fresh_read(Self::frequency_native)
    }

    fn frequency_native(&mut self) -> Result<u64, Error> {
        let handle = self.pointer()?;
        let mut frequency = 0.0;
        // SAFETY: the owner provides a complete writable frequency result.
        status(unsafe { raw::rig_get_freq(handle, raw::RPTADV_VFO_CURRENT, &mut frequency) })?;
        if !frequency.is_finite()
            || frequency <= 0.0
            || frequency > (1_u64 << 53) as f64
            || frequency.fract() != 0.0
        {
            return Err(Error::InvalidState);
        }
        Ok(frequency as u64)
    }

    /// Apply PTT only when this adapter is its explicitly selected source.
    pub fn set_ptt(&mut self, keyed: bool) -> Result<(), Error> {
        let handle = self.pointer()?;
        self.owns_ptt = true;
        // SAFETY: the unique owner supplies a valid enum and current VFO.
        status(unsafe {
            raw::rig_set_ptt(
                handle,
                raw::RPTADV_VFO_CURRENT,
                if keyed {
                    raw::RIG_PTT_ON
                } else {
                    raw::RIG_PTT_OFF
                },
            )
        })
    }

    /// Return the rig's PTT observation, not a cached key intent.
    pub fn ptt(&mut self) -> Result<bool, Error> {
        if !self.ptt_observable()? {
            return Err(Error::InvalidState);
        }
        self.fresh_read(Self::ptt_native)
    }

    fn ptt_native(&mut self) -> Result<bool, Error> {
        let handle = self.pointer()?;
        let mut keyed = raw::RIG_PTT_OFF;
        // SAFETY: the current owner supplies the writable native enum.
        status(unsafe { raw::rig_get_ptt(handle, raw::RPTADV_VFO_CURRENT, &mut keyed) })?;
        match keyed {
            raw::RIG_PTT_OFF => Ok(false),
            raw::RIG_PTT_ON | raw::RIG_PTT_ON_MIC | raw::RIG_PTT_ON_DATA => Ok(true),
            _ => Err(Error::InvalidState),
        }
    }

    /// Stop this adapter's PTT, close transport, then release frontend state.
    pub fn close(&mut self) -> Result<(), Error> {
        if self.handle.is_none() {
            return Ok(());
        }
        let handle = self.pointer()?;
        let mut result = Ok(());
        if self.opened {
            if self.owns_ptt {
                result = self.set_ptt(false);
            }
            // SAFETY: this is the sole owner closing its opened transport.
            let closed = {
                let _guard = FRONTEND
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                status(unsafe { raw::rig_close(handle) })
            };
            result = result.and(closed);
        }
        self.opened = false;
        self.handle = None;
        let _guard = FRONTEND
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // SAFETY: transport is closed and the live frontend handle is consumed once.
        result.and(status(unsafe { raw::rig_cleanup(handle) }))
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
