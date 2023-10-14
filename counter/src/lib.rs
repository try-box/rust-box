#[cfg(any(feature = "count", feature = "rate"))]
#[macro_use]
pub extern crate serde;

#[cfg(any(feature = "count", feature = "rate"))]
pub mod counter;
#[cfg(any(feature = "count", feature = "rate"))]
pub use counter::Counter;
#[cfg(any(feature = "count", feature = "rate"))]
pub use counter::LocalCounter;
