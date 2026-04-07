use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};

use super::to_io_err;
use crate::io::{self, AsyncInputStream, AsyncOutputStream};

#[cfg(wstd_p2)]
mod p2 {
    use super::*;
    use crate::runtime::AsyncPollable;
    use wasip2::sockets::instance_network::instance_network;
    use wasip2::sockets::network::Ipv4SocketAddress;
    use wasip2::sockets::tcp::{IpAddressFamily, IpSocketAddress};
    use wasip2::sockets::tcp_create_socket::create_tcp_socket;
    use wasip2::{
        io::streams::{InputStream, OutputStream},
        sockets::tcp::TcpSocket,
    };

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
            let network = instance_network();

            let remote_address = match addr {
                SocketAddr::V4(addr) => {
                    let ip = addr.ip().octets();
                    let address = (ip[0], ip[1], ip[2], ip[3]);
                    let port = addr.port();
                    IpSocketAddress::Ipv4(Ipv4SocketAddress { port, address })
                }
                SocketAddr::V6(_) => todo!("IPv6 not yet supported in `wstd::net::TcpStream`"),
            };
            socket
                .start_connect(&network, remote_address)
                .map_err(to_io_err)?;
            let pollable = AsyncPollable::new(socket.subscribe());
            pollable.wait_for().await;
            let (input, output) = socket.finish_connect().map_err(to_io_err)?;

            Ok(TcpStream::new(input, output, socket))
        }

        /// Returns the socket address of the remote peer of this TCP connection.
        pub fn peer_addr(&self) -> io::Result<String> {
            let addr = self.socket.remote_address().map_err(to_io_err)?;
            Ok(format!("{addr:?}"))
        }

        pub fn split(&self) -> (ReadHalf<'_>, WriteHalf<'_>) {
            (ReadHalf(self), WriteHalf(self))
        }
    }

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

        fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
            Some(&self.input)
        }
    }

    impl io::AsyncRead for &TcpStream {
        async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf).await
        }

        fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
            (**self).as_async_input_stream()
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

    impl io::AsyncWrite for &TcpStream {
        async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.write(buf).await
        }

        async fn flush(&mut self) -> io::Result<()> {
            self.output.flush().await
        }

        fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
            (**self).as_async_output_stream()
        }
    }

    pub struct ReadHalf<'a>(&'a TcpStream);
    impl<'a> io::AsyncRead for ReadHalf<'a> {
        async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf).await
        }

        fn as_async_input_stream(&self) -> Option<&AsyncInputStream> {
            self.0.as_async_input_stream()
        }
    }

    impl<'a> Drop for ReadHalf<'a> {
        fn drop(&mut self) {
            let _ = self
                .0
                .socket
                .shutdown(wasip2::sockets::tcp::ShutdownType::Receive);
        }
    }

    pub struct WriteHalf<'a>(&'a TcpStream);
    impl<'a> io::AsyncWrite for WriteHalf<'a> {
        async fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.write(buf).await
        }

        async fn flush(&mut self) -> io::Result<()> {
            self.0.flush().await
        }

        fn as_async_output_stream(&self) -> Option<&AsyncOutputStream> {
            self.0.as_async_output_stream()
        }
    }

    impl<'a> Drop for WriteHalf<'a> {
        fn drop(&mut self) {
            let _ = self
                .0
                .socket
                .shutdown(wasip2::sockets::tcp::ShutdownType::Send);
        }
    }
}

#[cfg(wstd_p2)]
pub use p2::*;

#[cfg(wstd_p3)]
mod p3 {
    use super::*;
    use wasip3::sockets::types::{IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket};

    /// A TCP stream between a local and a remote socket.
    pub struct TcpStream {
        input: AsyncInputStream,
        output: AsyncOutputStream,
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

            // The socket is consumed here — the streams and completion futures
            // keep the underlying connection alive via their handles.
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
}

#[cfg(wstd_p3)]
pub use p3::*;
