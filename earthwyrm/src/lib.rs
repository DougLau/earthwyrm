// lib.rs
//
// Copyright (c) 2026  Douglas Lau
//
#![forbid(unsafe_code)]

pub mod error;
mod fetch;
mod map;

pub use fetch::Uri;
pub use map::Map;
