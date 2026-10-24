//! Protocol layer: message types, request builders and parsers. No networking, no clock, no threads. The "code size" criterion of
//! the spike counts only this directory.

pub mod rest;
pub mod socket;
