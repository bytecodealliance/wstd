#[cfg(target_env = "p2")]
use wasip2::sockets::tcp::{IpAddressFamily, TcpSocket};
#[cfg(target_env = "p3")]
use wasip3::{
    sockets::types::{IpAddressFamily, TcpSocket},
    wit_bindgen::StreamReader,
};

use crate::io;
use crate::iter::AsyncIterator;
use std::net::SocketAddr;

use super::{TcpStream, create_tcp_socket, sockaddr_from_wasi, sockaddr_to_wasi, to_io_err};
#[cfg(target_env = "p2")]
use crate::runtime::AsyncPollable;

/// A TCP socket server, listening for connections.
#[derive(Debug)]
pub struct TcpListener {
    // Field order matters: must drop this child before parent below
    #[cfg(target_env = "p2")]
    pollable: AsyncPollable,
    #[cfg(target_env = "p3")]
    connections: StreamReader<TcpSocket>,
    socket: TcpSocket,
}

impl TcpListener {
    /// Creates a new TcpListener which will be bound to the specified address.
    ///
    /// The returned listener is ready for accepting connections.
    pub async fn bind(addr: &str) -> io::Result<Self> {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|_| io::Error::other("failed to parse string to socket addr"))?;
        let family = match addr {
            SocketAddr::V4(_) => IpAddressFamily::Ipv4,
            SocketAddr::V6(_) => IpAddressFamily::Ipv6,
        };
        let socket = create_tcp_socket(family).map_err(to_io_err)?;
        let local_address = sockaddr_to_wasi(addr);

        #[cfg(target_env = "p2")]
        {
            let network = wasip2::sockets::instance_network::instance_network();
            socket
                .start_bind(&network, local_address)
                .map_err(to_io_err)?;
            let pollable = AsyncPollable::new(socket.subscribe());
            pollable.wait_for().await;
            socket.finish_bind().map_err(to_io_err)?;

            socket.start_listen().map_err(to_io_err)?;
            pollable.wait_for().await;
            socket.finish_listen().map_err(to_io_err)?;
            Ok(Self { pollable, socket })
        }
        #[cfg(target_env = "p3")]
        {
            socket.bind(local_address).map_err(to_io_err)?;
            let connections = socket.listen().map_err(to_io_err)?;
            Ok(Self {
                connections,
                socket,
            })
        }
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
        #[cfg(target_env = "p2")]
        let addr = self.socket.local_address();
        #[cfg(target_env = "p3")]
        let addr = self.socket.get_local_address();
        addr.map_err(to_io_err).map(sockaddr_from_wasi)
    }

    /// Returns an iterator over the connections being received on this listener.
    pub fn incoming(&mut self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}

/// An iterator that infinitely accepts connections on a TcpListener.
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a mut TcpListener,
}

impl<'a> AsyncIterator for Incoming<'a> {
    type Item = io::Result<TcpStream>;

    #[cfg(target_env = "p2")]
    async fn next(&mut self) -> Option<Self::Item> {
        self.listener.pollable.wait_for().await;
        let (socket, input, output) = match self.listener.socket.accept().map_err(to_io_err) {
            Ok(accepted) => accepted,
            Err(err) => return Some(Err(err)),
        };
        Some(Ok(TcpStream::new(input, output, socket)))
    }

    #[cfg(target_env = "p3")]
    async fn next(&mut self) -> Option<Self::Item> {
        self.listener.connections.next().await.map(|socket| {
            let (input, _receive_result) = socket.receive();
            let (output, receiver) = wasip3::wit_stream::new();
            let _send_result = socket.send(receiver);
            Ok(TcpStream::new(input, output, socket))
        })
    }
}
