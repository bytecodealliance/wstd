//! Async network abstractions.

use std::io::{self, ErrorKind};

mod tcp_listener;
mod tcp_stream;

pub use tcp_listener::*;
pub use tcp_stream::*;

#[cfg(wstd_p2)]
fn to_io_err(err: wasip2::sockets::network::ErrorCode) -> io::Error {
    use wasip2::sockets::network::ErrorCode;
    match err {
        ErrorCode::Unknown => ErrorKind::Other.into(),
        ErrorCode::AccessDenied => ErrorKind::PermissionDenied.into(),
        ErrorCode::NotSupported => ErrorKind::Unsupported.into(),
        ErrorCode::InvalidArgument => ErrorKind::InvalidInput.into(),
        ErrorCode::OutOfMemory => ErrorKind::OutOfMemory.into(),
        ErrorCode::Timeout => ErrorKind::TimedOut.into(),
        ErrorCode::WouldBlock => ErrorKind::WouldBlock.into(),
        ErrorCode::InvalidState => ErrorKind::InvalidData.into(),
        ErrorCode::AddressInUse => ErrorKind::AddrInUse.into(),
        ErrorCode::ConnectionRefused => ErrorKind::ConnectionRefused.into(),
        ErrorCode::ConnectionReset => ErrorKind::ConnectionReset.into(),
        ErrorCode::ConnectionAborted => ErrorKind::ConnectionAborted.into(),
        ErrorCode::ConcurrencyConflict => ErrorKind::AlreadyExists.into(),
        _ => ErrorKind::Other.into(),
    }
}

#[cfg(wstd_p3)]
fn to_io_err(err: wasip3::sockets::types::ErrorCode) -> io::Error {
    use wasip3::sockets::types::ErrorCode;
    match err {
        ErrorCode::AccessDenied => ErrorKind::PermissionDenied.into(),
        ErrorCode::NotSupported => ErrorKind::Unsupported.into(),
        ErrorCode::InvalidArgument => ErrorKind::InvalidInput.into(),
        ErrorCode::OutOfMemory => ErrorKind::OutOfMemory.into(),
        ErrorCode::Timeout => ErrorKind::TimedOut.into(),
        ErrorCode::InvalidState => ErrorKind::InvalidData.into(),
        ErrorCode::AddressInUse => ErrorKind::AddrInUse.into(),
        ErrorCode::ConnectionRefused => ErrorKind::ConnectionRefused.into(),
        ErrorCode::ConnectionReset => ErrorKind::ConnectionReset.into(),
        ErrorCode::ConnectionAborted => ErrorKind::ConnectionAborted.into(),
        ErrorCode::RemoteUnreachable => ErrorKind::HostUnreachable.into(),
        _ => ErrorKind::Other.into(),
    }
}
