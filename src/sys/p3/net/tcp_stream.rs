use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};

use super::to_io_err;
use crate::io::{self, AsyncInputStream, AsyncOutputStream};
use wasip3::sockets::types::{IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket};

/// A TCP stream between a local and a remote socket.
pub struct TcpStream {
    input: AsyncInputStream,
    output: AsyncOutputStream,
}

impl std::fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TcpStream").finish()
    }
}

impl TcpStream {
    pub(crate) fn new(input: AsyncInputStream, output: AsyncOutputStream) -> Self {
        TcpStream { input, output }
    }

    /// Opens a TCP connection to a remote host.
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
        let socket = TcpSocket::create(family).map_err(to_io_err)?;

        let remote_address = match addr {
            SocketAddr::V4(addr) => {
                let ip = addr.ip().octets();
                let address = (ip[0], ip[1], ip[2], ip[3]);
                let port = addr.port();
                IpSocketAddress::Ipv4(Ipv4SocketAddress { port, address })
            }
            SocketAddr::V6(_) => todo!("IPv6 not yet supported in `wstd::net::TcpStream`"),
        };

        // p3 connect is async
        socket.connect(remote_address).await.map_err(to_io_err)?;

        Self::from_connected_socket(socket)
    }

    /// Create a TcpStream from an already-connected socket.
    pub(crate) fn from_connected_socket(socket: TcpSocket) -> io::Result<Self> {
        // Get receive stream
        let (recv_reader, _recv_completion) = socket.receive();
        let input = AsyncInputStream::new(recv_reader);

        // Create a send stream and wire it to the socket
        let (send_writer, send_reader) = wasip3::wit_stream::new::<u8>();
        let _send_completion = socket.send(send_reader);
        let output = AsyncOutputStream::new(send_writer);

        Ok(TcpStream::new(input, output))
    }

    pub fn split(&mut self) -> (ReadHalf<'_>, WriteHalf<'_>) {
        let ptr = self as *mut TcpStream;
        // Safety: ReadHalf only accesses input, WriteHalf only accesses output
        #[allow(unsafe_code)]
        unsafe {
            (ReadHalf(&mut *ptr), WriteHalf(&mut *ptr))
        }
    }
}

impl io::AsyncRead for TcpStream {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.input.read(buf).await
    }

    fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
        Some(&self.input)
    }
}

impl io::AsyncWrite for TcpStream {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.output.write(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.output.flush().await
    }

    fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
        Some(&self.output)
    }
}

pub struct ReadHalf<'a>(&'a mut TcpStream);
impl<'a> io::AsyncRead for ReadHalf<'a> {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.input.read(buf).await
    }

    fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
        Some(&self.0.input)
    }
}

pub struct WriteHalf<'a>(&'a mut TcpStream);
impl<'a> io::AsyncWrite for WriteHalf<'a> {
    async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.output.write(buf).await
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.0.output.flush().await
    }

    fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
        Some(&self.0.output)
    }
}
