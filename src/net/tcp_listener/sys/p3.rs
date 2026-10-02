use wasip3::{
    sockets::types::{IpAddressFamily, TcpSocket},
    wit_bindgen::StreamReader,
};

use crate::io;
use crate::iter::AsyncIterator;
use std::net::SocketAddr;

use crate::net::{TcpStream, create_tcp_socket, sockaddr_from_wasi, sockaddr_to_wasi, to_io_err};

/// A TCP socket server, listening for connections.
#[derive(Debug)]
pub struct TcpListener {
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

        socket.bind(local_address).map_err(to_io_err)?;
        let connections = socket.listen().map_err(to_io_err)?;
        Ok(Self {
            connections,
            socket,
        })
    }

    /// Returns the local socket address of this listener.
    pub fn local_addr(&self) -> io::Result<std::net::SocketAddr> {
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

    async fn next(&mut self) -> Option<Self::Item> {
        self.listener.connections.next().await.map(|socket| {
            let (input, _receive_result) = socket.receive();
            let (output, receiver) = wasip3::wit_stream::new();
            let _send_result = socket.send(receiver);
            Ok(TcpStream::new(input, output, socket))
        })
    }
}
