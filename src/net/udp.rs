use std::io::ErrorKind;
use std::net::{SocketAddr, ToSocketAddrs};
#[cfg(target_env = "p2")]
use std::sync::OnceLock;

#[cfg(target_env = "p2")]
use wasip2::sockets::{
    instance_network::instance_network,
    udp::{
        IncomingDatagramStream, IpAddressFamily, IpSocketAddress, OutgoingDatagram,
        OutgoingDatagramStream, UdpSocket as WasiUdpSocket,
    },
};
#[cfg(target_env = "p3")]
use wasip3::sockets::types::{IpAddressFamily, UdpSocket as WasiUdpSocket};

use super::{create_udp_socket, sockaddr_from_wasi, sockaddr_to_wasi, to_io_err};
use crate::io;
#[cfg(target_env = "p2")]
use crate::runtime::AsyncPollable;

/// A UDP socket, bound to a local address.
///
/// A `UdpSocket` is not associated with any remote address, so datagrams can be
/// sent to, and received from, any address, using [`UdpSocket::send_to`] and
/// [`UdpSocket::recv_from`]. Use [`UdpSocket::connect`] to associate it with a
/// single remote address instead, giving a [`UdpStream`].
#[derive(Debug)]
pub struct UdpSocket {
    #[cfg(target_env = "p2")]
    incoming: AsyncIncomingDatagramStream,
    #[cfg(target_env = "p2")]
    outgoing: AsyncOutgoingDatagramStream,
    socket: WasiUdpSocket,
}

impl UdpSocket {
    /// Creates a new UdpSocket bound to the specified local address.
    pub async fn bind(addr: &str) -> io::Result<Self> {
        let addr: SocketAddr = addr
            .parse()
            .map_err(|_| io::Error::other("failed to parse string to socket addr"))?;
        let socket = bind_socket(addr).await?;

        #[cfg(target_env = "p2")]
        {
            // Datagram streams without a remote address may send to, and receive
            // from, any address.
            let (incoming, outgoing) = socket.stream(None).map_err(to_io_err)?;
            Ok(Self {
                incoming: AsyncIncomingDatagramStream::new(incoming),
                outgoing: AsyncOutgoingDatagramStream::new(outgoing),
                socket,
            })
        }
        #[cfg(target_env = "p3")]
        Ok(Self { socket })
    }

    /// Returns the local socket address of this socket.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        #[cfg(target_env = "p2")]
        let addr = self.socket.local_address();
        #[cfg(target_env = "p3")]
        let addr = self.socket.get_local_address();
        addr.map_err(to_io_err).map(sockaddr_from_wasi)
    }

    /// Sends a datagram to the given address.
    pub async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> io::Result<usize> {
        #[cfg(target_env = "p2")]
        return self
            .outgoing
            .send_to(buf, Some(sockaddr_to_wasi(addr)))
            .await;
        #[cfg(target_env = "p3")]
        {
            self.socket
                .send(buf.to_vec(), Some(sockaddr_to_wasi(addr)))
                .await
                .map_err(to_io_err)?;
            Ok(buf.len())
        }
    }

    /// Receives a single datagram. On success, returns the number of bytes
    /// received and the address the datagram was sent from.
    ///
    /// If `buf` is shorter than the datagram, the excess bytes are discarded.
    pub async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        #[cfg(target_env = "p2")]
        return self.incoming.recv_from(buf).await;
        #[cfg(target_env = "p3")]
        {
            let (datagram, remote_address) = self.socket.receive().await.map_err(to_io_err)?;
            let len = datagram.len().min(buf.len());
            buf[..len].copy_from_slice(&datagram[..len]);
            Ok((len, sockaddr_from_wasi(remote_address)))
        }
    }

    /// Associates this socket with a remote address, giving a [`UdpStream`]
    /// which sends to, and receives from, only that address.
    ///
    /// This only changes the local socket configuration, and does not generate
    /// any network traffic.
    pub fn connect(self, addr: SocketAddr) -> io::Result<UdpStream> {
        #[cfg(target_env = "p2")]
        {
            // WASI may trap if streams from a previous call to `stream` are still
            // live, so drop the unconnected streams before creating connected ones.
            let Self {
                incoming,
                outgoing,
                socket,
            } = self;
            drop((incoming, outgoing));

            let (incoming, outgoing) = socket
                .stream(Some(sockaddr_to_wasi(addr)))
                .map_err(to_io_err)?;
            Ok(UdpStream::new(incoming, outgoing, socket))
        }
        #[cfg(target_env = "p3")]
        {
            self.socket
                .connect(sockaddr_to_wasi(addr))
                .map_err(to_io_err)?;
            Ok(UdpStream::new(self.socket))
        }
    }

    /// Returns the unicast hop limit ("time to live") of this socket.
    pub fn unicast_hop_limit(&self) -> io::Result<u8> {
        #[cfg(target_env = "p2")]
        let result = self.socket.unicast_hop_limit();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_unicast_hop_limit();
        result.map_err(to_io_err)
    }

    /// Sets the unicast hop limit ("time to live") of this socket.
    pub fn set_unicast_hop_limit(&self, value: u8) -> io::Result<()> {
        self.socket.set_unicast_hop_limit(value).map_err(to_io_err)
    }

    /// Returns the size of the receive buffer of this socket.
    pub fn receive_buffer_size(&self) -> io::Result<u64> {
        #[cfg(target_env = "p2")]
        let result = self.socket.receive_buffer_size();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_receive_buffer_size();
        result.map_err(to_io_err)
    }

    /// Sets the size of the receive buffer of this socket. This is a hint: the
    /// size reported by [`UdpSocket::receive_buffer_size`] may differ.
    pub fn set_receive_buffer_size(&self, value: u64) -> io::Result<()> {
        self.socket
            .set_receive_buffer_size(value)
            .map_err(to_io_err)
    }

    /// Returns the size of the send buffer of this socket.
    pub fn send_buffer_size(&self) -> io::Result<u64> {
        #[cfg(target_env = "p2")]
        let result = self.socket.send_buffer_size();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_send_buffer_size();
        result.map_err(to_io_err)
    }

    /// Sets the size of the send buffer of this socket. This is a hint: the
    /// size reported by [`UdpSocket::send_buffer_size`] may differ.
    pub fn set_send_buffer_size(&self, value: u64) -> io::Result<()> {
        self.socket.set_send_buffer_size(value).map_err(to_io_err)
    }
}

/// A UDP socket associated with a remote address.
///
/// A `UdpStream` sends to, and receives from, only the address it was connected
/// to, using [`UdpStream::send`] and [`UdpStream::recv`]. Datagrams sent from
/// any other address are not received.
#[derive(Debug)]
pub struct UdpStream {
    #[cfg(target_env = "p2")]
    incoming: AsyncIncomingDatagramStream,
    #[cfg(target_env = "p2")]
    outgoing: AsyncOutgoingDatagramStream,
    socket: WasiUdpSocket,
}

impl UdpStream {
    #[cfg(target_env = "p2")]
    fn new(
        incoming: IncomingDatagramStream,
        outgoing: OutgoingDatagramStream,
        socket: WasiUdpSocket,
    ) -> Self {
        Self {
            incoming: AsyncIncomingDatagramStream::new(incoming),
            outgoing: AsyncOutgoingDatagramStream::new(outgoing),
            socket,
        }
    }

    #[cfg(target_env = "p3")]
    fn new(socket: WasiUdpSocket) -> Self {
        Self { socket }
    }

    /// Associates a UDP socket with a remote host.
    pub async fn connect(addr: impl ToSocketAddrs) -> io::Result<Self> {
        let addrs = addr.to_socket_addrs()?;
        let mut last_err = None;
        for addr in addrs {
            match UdpStream::connect_addr(addr).await {
                Ok(stream) => return Ok(stream),
                Err(e) => last_err = Some(e),
            }
        }

        Err(last_err.unwrap_or_else(|| {
            io::Error::new(ErrorKind::InvalidInput, "could not resolve to any address")
        }))
    }

    /// Establishes an association with the specified `addr`.
    pub async fn connect_addr(addr: SocketAddr) -> io::Result<Self> {
        // Unlike in POSIX, WASI requires a UDP socket be explicitly bound
        // before it can be associated with a remote address. Bind to the
        // unspecified address of the same family, and let the host choose a
        // port.
        let local_addr = match addr {
            SocketAddr::V4(_) => SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0)),
            SocketAddr::V6(_) => SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, 0)),
        };
        let socket = bind_socket(local_addr).await?;

        #[cfg(target_env = "p2")]
        {
            let (incoming, outgoing) = socket
                .stream(Some(sockaddr_to_wasi(addr)))
                .map_err(to_io_err)?;
            Ok(Self::new(incoming, outgoing, socket))
        }
        #[cfg(target_env = "p3")]
        {
            socket.connect(sockaddr_to_wasi(addr)).map_err(to_io_err)?;
            Ok(Self::new(socket))
        }
    }

    /// Returns the local socket address of this socket.
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        #[cfg(target_env = "p2")]
        let addr = self.socket.local_address();
        #[cfg(target_env = "p3")]
        let addr = self.socket.get_local_address();
        addr.map_err(to_io_err).map(sockaddr_from_wasi)
    }

    /// Returns the socket address of the remote peer of this UDP association.
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        #[cfg(target_env = "p2")]
        let addr = self.socket.remote_address();
        #[cfg(target_env = "p3")]
        let addr = self.socket.get_remote_address();
        addr.map_err(to_io_err).map(sockaddr_from_wasi)
    }

    /// Sends a datagram to the remote peer.
    pub async fn send(&self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(target_env = "p2")]
        return self.outgoing.send_to(buf, None).await;
        #[cfg(target_env = "p3")]
        {
            self.socket
                .send(buf.to_vec(), None)
                .await
                .map_err(to_io_err)?;
            Ok(buf.len())
        }
    }

    /// Receives a single datagram from the remote peer. On success, returns the
    /// number of bytes received.
    ///
    /// If `buf` is shorter than the datagram, the excess bytes are discarded.
    pub async fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        #[cfg(target_env = "p2")]
        return self.incoming.recv_from(buf).await.map(|(len, _addr)| len);
        #[cfg(target_env = "p3")]
        {
            let (datagram, _remote_address) = self.socket.receive().await.map_err(to_io_err)?;
            let len = datagram.len().min(buf.len());
            buf[..len].copy_from_slice(&datagram[..len]);
            Ok(len)
        }
    }

    /// Returns the unicast hop limit ("time to live") of this socket.
    pub fn unicast_hop_limit(&self) -> io::Result<u8> {
        #[cfg(target_env = "p2")]
        let result = self.socket.unicast_hop_limit();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_unicast_hop_limit();
        result.map_err(to_io_err)
    }

    /// Sets the unicast hop limit ("time to live") of this socket.
    pub fn set_unicast_hop_limit(&self, value: u8) -> io::Result<()> {
        self.socket.set_unicast_hop_limit(value).map_err(to_io_err)
    }

    /// Returns the size of the receive buffer of this socket.
    pub fn receive_buffer_size(&self) -> io::Result<u64> {
        #[cfg(target_env = "p2")]
        let result = self.socket.receive_buffer_size();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_receive_buffer_size();
        result.map_err(to_io_err)
    }

    /// Sets the size of the receive buffer of this socket. This is a hint: the
    /// size reported by [`UdpStream::receive_buffer_size`] may differ.
    pub fn set_receive_buffer_size(&self, value: u64) -> io::Result<()> {
        self.socket
            .set_receive_buffer_size(value)
            .map_err(to_io_err)
    }

    /// Returns the size of the send buffer of this socket.
    pub fn send_buffer_size(&self) -> io::Result<u64> {
        #[cfg(target_env = "p2")]
        let result = self.socket.send_buffer_size();
        #[cfg(target_env = "p3")]
        let result = self.socket.get_send_buffer_size();
        result.map_err(to_io_err)
    }

    /// Sets the size of the send buffer of this socket. This is a hint: the
    /// size reported by [`UdpStream::send_buffer_size`] may differ.
    pub fn set_send_buffer_size(&self, value: u64) -> io::Result<()> {
        self.socket.set_send_buffer_size(value).map_err(to_io_err)
    }
}

async fn bind_socket(addr: SocketAddr) -> io::Result<WasiUdpSocket> {
    let family = match addr {
        SocketAddr::V4(_) => IpAddressFamily::Ipv4,
        SocketAddr::V6(_) => IpAddressFamily::Ipv6,
    };
    let socket = create_udp_socket(family).map_err(to_io_err)?;
    let local_address = sockaddr_to_wasi(addr);

    #[cfg(target_env = "p2")]
    {
        let network = instance_network();
        socket
            .start_bind(&network, local_address)
            .map_err(to_io_err)?;
        let pollable = AsyncPollable::new(socket.subscribe());
        pollable.wait_for().await;
        socket.finish_bind().map_err(to_io_err)?;
    }
    #[cfg(target_env = "p3")]
    socket.bind(local_address).map_err(to_io_err)?;

    Ok(socket)
}

#[cfg(target_env = "p2")]
#[derive(Debug)]
struct AsyncIncomingDatagramStream {
    subscription: OnceLock<AsyncPollable>,
    stream: IncomingDatagramStream,
}

#[cfg(target_env = "p2")]
impl AsyncIncomingDatagramStream {
    fn new(stream: IncomingDatagramStream) -> Self {
        Self {
            subscription: OnceLock::new(),
            stream,
        }
    }

    /// Await receive readiness.
    async fn ready(&self) {
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        subscription.wait_for().await;
    }

    /// Asynchronous receive.
    async fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let datagram = loop {
            self.ready().await;
            match self
                .stream
                .receive(1)
                .map_err(to_io_err)?
                .into_iter()
                .next()
            {
                Some(datagram) => break datagram,
                // `self.ready()` cannot guarantee that a datagram is ready to
                // receive, so try again if WASI returns an empty list.
                None => continue,
            }
        };
        let len = datagram.data.len().min(buf.len());
        buf[0..len].copy_from_slice(&datagram.data[0..len]);
        Ok((len, sockaddr_from_wasi(datagram.remote_address)))
    }
}

#[cfg(target_env = "p2")]
#[derive(Debug)]
struct AsyncOutgoingDatagramStream {
    subscription: OnceLock<AsyncPollable>,
    stream: OutgoingDatagramStream,
}

#[cfg(target_env = "p2")]
impl AsyncOutgoingDatagramStream {
    fn new(stream: OutgoingDatagramStream) -> Self {
        Self {
            subscription: OnceLock::new(),
            stream,
        }
    }

    /// Await send readiness.
    async fn ready(&self) {
        let subscription = self
            .subscription
            .get_or_init(|| AsyncPollable::new(self.stream.subscribe()));
        subscription.wait_for().await;
    }

    /// Asynchronous send.
    async fn send_to(
        &self,
        buf: &[u8],
        remote_address: Option<IpSocketAddress>,
    ) -> io::Result<usize> {
        let datagrams = [OutgoingDatagram {
            data: buf.to_vec(),
            remote_address,
        }];
        loop {
            if self.stream.check_send().map_err(to_io_err)? == 0 {
                self.ready().await;
                continue;
            }
            if self.stream.send(&datagrams).map_err(to_io_err)? == 1 {
                return Ok(buf.len());
            }
        }
    }
}
