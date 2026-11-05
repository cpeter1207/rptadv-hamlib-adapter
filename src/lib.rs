//! Versioned adapter to released Hamlib; no owned policy or PCM processing.

#![deny(warnings)]

#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::ptr_offset_with_cast,
    clippy::upper_case_acronyms,
    clippy::useless_transmute,
    clippy::too_many_arguments
)]
// Mechanically generated external C declarations retain their naming and
// bitfield helpers. All authored adapter code remains warning-clean.
mod raw {
    include!(concat!(env!("OUT_DIR"), "/hamlib.rs"));
}

mod rig;
pub use rig::*;
mod catalog;
pub use catalog::*;
mod ffi;
pub use ffi::rptadv_hamlib_api_v1;

#[cfg(test)]
mod tests;
