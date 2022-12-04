#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std as alloc;

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod map;

#[cfg(feature = "serde")]
mod serde;

pub use map::DequeMap;
