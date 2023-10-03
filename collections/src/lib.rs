#![deny(unsafe_code)]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(feature = "std")]
extern crate std as alloc;

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "binary-heap")]
pub mod binary_heap;
#[cfg(feature = "binary-heap")]
pub use binary_heap::BinaryHeap;

#[cfg(feature = "priority-queue")]
pub mod priority_queue;
#[cfg(feature = "priority-queue")]
pub use priority_queue::PriorityQueue;
