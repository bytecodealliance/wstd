use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};

#[cfg(target_env = "p2")]
use wasip2::{
    io::streams::{InputStream, OutputStream},
    sockets::{
        instance_network::instance_network,
        network::Ipv4SocketAddress,
        tcp::{IpAddressFamily, IpSocketAddress, TcpSocket},
    },
};

#[cfg(target_env = "p3")]
use wasip3::sockets::types::{IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket};
#[cfg(target_env = "p3")]
type InputStream = wasip3::wit_bindgen::StreamReader<u8>;
#[cfg(target_env = "p3")]
type OutputStream = wasip3::wit_bindgen::StreamWriter<u8>;

use super::{create_tcp_socket, to_io_err};
use crate::io::{self, AsyncInputStream, AsyncOutputStream};
#[cfg(target_env = "p2")]
use crate::runtime::AsyncPollable;

/// A TCP stream between a local and a remote socket.
pub struct TcpStream {
    input: AsyncInputStream,
    output: AsyncOutputStream,
    socket: TcpSocket,
}

impl TcpStream {
    pub(crate) fn new(input: InputStream, output: OutputStream, socket: TcpSocket) -> Self {
        TcpStream {
            input: AsyncInputStream::new(input),
            output: AsyncOutputStream::new(output),
            socket,
        }
    }

    /// Opens a TCP connection to a remote host.
    ///
    /// `addr` is an address of the remote host. Anything which implements the
    /// [`ToSocketAddrs`] trait can be supplied as the address.  If `addr`
    /// yields multiple addresses, connect will be attempted with each of the
    /// addresses until a connection is successful. If none of the addresses
    /// result in a successful connection, the error returned from the last
    /// connection attempt (the last address) is returned.
    pub async fn connect(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;
        for addr in addrs {
            match TcpStream::connect_addr(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(ErrorKind::InvalidInput, "could not resolve to any address")
        }))
    }

    /// Establishes a connection to the specified `addr`.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        let family = match addr {
            SocketAddr::V4(_) => IpAddressFamily::Ipv4,
            SocketAddr::V6(_) => IpAddressFamily::Ipv6,
        };
        let socket = create_tcp_socket(family).map_err(to_io_err)?;

        let remote_address = match addr {
            SocketAddr::V4(addr) => {
                let ip = addr.ip().octets();
                let address = (ip[0], ip[1], ip[2], ip[3]);
                let port = addr.port();
                IpSocketAddress::Ipv4(Ipv4SocketAddress { port, address })
            }
            SocketAddr::V6(_) => todo!("IPv6 not yet supported in `wstd::net::TcpStream`"),
        };
        #[cfg(target_env = "p2")]
        {
            let network = instance_network();
            socket
                .start_connect(&network, remote_address)
                .map_err(to_io_err)?;
            let pollable = AsyncPollable::new(socket.subscribe());
            pollable.wait_for().await;
            let (input, output) = socket.finish_connect().map_err(to_io_err)?;
            Ok(TcpStream::new(input, output, socket))
        }
        #[cfg(target_env = "p3")]
        {
            socket.connect(remote_address).await.map_err(to_io_err)?;
            let (input, _receive_result) = socket.receive();
            let (output, receiver) = wasip3::wit_stream::new();
            let _send_result = socket.send(receiver);
            Ok(TcpStream::new(input, output, socket))
        }
    }

    /// Returns the socket address of the remote peer of this TCP connection.
    pub fn peer_addr(&self) -> io::Result<String> {
        #[cfg(target_env = "p2")]
        let addr = self.socket.remote_address().map_err(to_io_err)?;
        #[cfg(target_env = "p3")]
        let addr = self.socket.get_remote_address().map_err(to_io_err)?;
        Ok(format!("{addr:?}"))
    }

    pub fn split(&mut self) -> (ReadHalf<'_>, WriteHalf<'_>) {
        (
            ReadHalf {
                stream: &mut self.input,
                #[cfg(target_env = "p2")]
                socket: &self.socket,
            },
            WriteHalf {
                stream: &mut self.output,
                #[cfg(target_env = "p2")]
                socket: &self.socket,
            },
        )
    }
}

#[cfg(target_env = "p2")]
impl Drop for TcpStream {
    fn drop(&mut self) {
        let _ = self
            .socket
            .shutdown(wasip2::sockets::tcp::ShutdownType::Both);
    }
}

impl io::AsyncRead for TcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf).await
    }

    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        Some(&mut self.input)
    }
}

impl io::AsyncWrite for TcpStream {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.write(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.output.flush().await
    }

    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        Some(&mut self.output)
    }
}

pub struct ReadHalf<'a> {
    stream: &'a mut AsyncInputStream,
    #[cfg(target_env = "p2")]
    socket: &'a TcpSocket,
}

#[cfg(target_env = "p2")]
impl<'a> Drop for ReadHalf<'a> {
    fn drop(&mut self) {
        let _ = self
            .socket
            .shutdown(wasip2::sockets::tcp::ShutdownType::Receive);
    }
}

impl<'a> io::AsyncRead for ReadHalf<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read(buf).await
    }

    fn as_async_input_stream(&mut self) -> Option<&mut AsyncInputStream> {
        self.stream.as_async_input_stream()
    }
}

pub struct WriteHalf<'a> {
    stream: &'a mut AsyncOutputStream,
    #[cfg(target_env = "p2")]
    socket: &'a TcpSocket,
}

impl<'a> io::AsyncWrite for WriteHalf<'a> {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream.write(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.stream.flush().await
    }

    fn as_async_output_stream(&mut self) -> Option<&mut AsyncOutputStream> {
        self.stream.as_async_output_stream()
    }
}

#[cfg(target_env = "p2")]
impl<'a> Drop for WriteHalf<'a> {
    fn drop(&mut self) {
        let _ = self
            .socket
            .shutdown(wasip2::sockets::tcp::ShutdownType::Send);
    }
}
