//! Async IO abstractions.

mod copy;
mod cursor;
mod empty;
mod read;
mod seek;
mod stdio;
#[cfg(target_env = "p2")]
mod streams_p2;
#[cfg(target_env = "p2")]
use streams_p2 as streams;
#[cfg(target_env = "p3")]
mod streams_p3;
#[cfg(target_env = "p3")]
use streams_p3 as streams;
mod write;

#[cfg(target_env = "p2")]
pub use crate::runtime::AsyncPollable;
pub use copy::*;
pub use cursor::*;
pub use empty::*;
pub use read::*;
pub use seek::*;
pub use stdio::*;
pub use streams::*;
pub use write::*;

/// The error type for I/O operations.
///
pub use std::io::Error;

/// A specialized Result type for I/O operations.
///
pub use std::io::Result;
