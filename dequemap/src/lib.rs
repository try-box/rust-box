#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

///This code is the main crate file for the deque-map crate. It includes a
///#![deny(unsafe_code)] directive, which tells the Rust compiler to treat all uses of
///unsafe code as a compilation error. This can help prevent the introduction of potential
///vulnerabilities into the code.
///
///The #![cfg_attr(not(feature = "std"), no_std)] directive tells the Rust compiler that the
///crate should not use the Rust standard library unless the "std" feature is enabled. This
///can be useful when building applications that need to operate in a resource-constrained
///environment, such as an embedded system.
///
///The extern crate declarations import the std and alloc crates. The std crate is only
///imported when the "std" feature is enabled, and the alloc crate is always imported. This
///allows the code to use either the Rust standard library or the alloc crate for memory
///allocation and related functionality, depending on the configuration of the crate.
///
///The pub mod map declaration exposes the map module as part of the crate's public API.
///This allows other crates to use the DequeMap type defined in the map module. The
///#[cfg(feature = "serde")] directive tells the Rust compiler to compile and include the
///serde module only if the "serde" feature is enabled. This allows the deque-map crate to
///support serialization and deserialization of DequeMap instances using the serde framework.
///
///Finally, the pub use map::DequeMap declaration re-exports the DequeMap type from the map
///module as part of the crate's public API. This allows other crates to use the DequeMap
///type without having to import it from the map module directly.
///
///The above content and some comments in the code are written by ChatGPT.

#[cfg(feature = "std")]
extern crate std as alloc;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "btreemap")]
pub mod btreemap;
#[cfg(feature = "btreemap")]
pub use btreemap::DequeBTreeMap;

#[cfg(feature = "hashmap")]
pub mod hashmap;
#[cfg(feature = "hashmap")]
pub use hashmap::DequeHashMap;
