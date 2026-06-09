//! Async network abstractions.
//!
//! The TCP types present a portable interface, but their implementations are
//! inherently platform-bound (socket handles, error-code mapping), so the
//! backend owns them in full and the facade pins the contract by name.

pub use crate::sys::net::{Incoming, ReadHalf, TcpListener, TcpStream, WriteHalf};
