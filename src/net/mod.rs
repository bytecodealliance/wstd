//! Async network abstractions.

use std::io::{self, ErrorKind};
#[cfg(target_env = "p2")]
use wasip2::sockets::{
    network::{ErrorCode, IpSocketAddress, Ipv4SocketAddress, Ipv6SocketAddress},
    tcp_create_socket::create_tcp_socket,
    udp_create_socket::create_udp_socket,
};
#[cfg(target_env = "p3")]
use wasip3::sockets::types::{
    ErrorCode, IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, Ipv6SocketAddress, TcpSocket,
};

mod tcp_listener;
mod tcp_stream;
mod udp;

pub use tcp_listener::*;
pub use tcp_stream::*;
pub use udp::*;

fn to_io_err(err: ErrorCode) -> io::Error {
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
        ErrorCode::DatagramTooLarge => ErrorKind::InvalidInput.into(),

        #[cfg(target_env = "p2")]
        ErrorCode::Unknown => ErrorKind::Other.into(),
        #[cfg(target_env = "p2")]
        ErrorCode::WouldBlock => ErrorKind::WouldBlock.into(),
        #[cfg(target_env = "p2")]
        ErrorCode::ConcurrencyConflict => ErrorKind::AlreadyExists.into(),
        #[cfg(target_env = "p2")]
        _ => ErrorKind::Other.into(),

        #[cfg(target_env = "p3")]
        ErrorCode::AddressNotBindable => ErrorKind::AddrNotAvailable.into(),
        #[cfg(target_env = "p3")]
        ErrorCode::RemoteUnreachable => ErrorKind::HostUnreachable.into(),
        #[cfg(target_env = "p3")]
        ErrorCode::ConnectionBroken => ErrorKind::BrokenPipe.into(),
        #[cfg(target_env = "p3")]
        ErrorCode::Other(s) => io::Error::other(s.unwrap_or_default()),
    }
}

fn sockaddr_from_wasi(addr: IpSocketAddress) -> std::net::SocketAddr {
    match addr {
        IpSocketAddress::Ipv4(Ipv4SocketAddress { address, port }) => {
            std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
                std::net::Ipv4Addr::new(address.0, address.1, address.2, address.3),
                port,
            ))
        }
        IpSocketAddress::Ipv6(Ipv6SocketAddress {
            address,
            port,
            flow_info,
            scope_id,
        }) => std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            std::net::Ipv6Addr::new(
                address.0, address.1, address.2, address.3, address.4, address.5, address.6,
                address.7,
            ),
            port,
            flow_info,
            scope_id,
        )),
    }
}

fn sockaddr_to_wasi(addr: std::net::SocketAddr) -> IpSocketAddress {
    match addr {
        std::net::SocketAddr::V4(addr) => {
            let ip = addr.ip().octets();
            IpSocketAddress::Ipv4(Ipv4SocketAddress {
                address: (ip[0], ip[1], ip[2], ip[3]),
                port: addr.port(),
            })
        }
        std::net::SocketAddr::V6(addr) => {
            let ip = addr.ip().segments();
            IpSocketAddress::Ipv6(Ipv6SocketAddress {
                address: (ip[0], ip[1], ip[2], ip[3], ip[4], ip[5], ip[6], ip[7]),
                port: addr.port(),
                flow_info: addr.flowinfo(),
                scope_id: addr.scope_id(),
            })
        }
    }
}

#[cfg(target_env = "p3")]
fn create_tcp_socket(
    family: IpAddressFamily,
) -> Result<TcpSocket, wasip3::sockets::types::ErrorCode> {
    TcpSocket::create(family)
}

#[cfg(target_env = "p3")]
fn create_udp_socket(
    family: IpAddressFamily,
) -> Result<wasip3::sockets::types::UdpSocket, wasip3::sockets::types::ErrorCode> {
    wasip3::sockets::types::UdpSocket::create(family)
}
