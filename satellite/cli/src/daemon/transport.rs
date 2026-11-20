use std::pin::Pin;

use mcpstore::error::{Error, FailureCode};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

/// Kernel RPC stream: local Unix socket or authenticated TCP.
pub enum HostStreamInner {
    Unix(tokio::net::UnixStream),
    Tcp(tokio::net::TcpStream),
}

#[derive(Debug)]
pub enum HostStreamOwnedRead {
    Unix(tokio::net::unix::OwnedReadHalf),
    Tcp(tokio::net::tcp::OwnedReadHalf),
}

#[derive(Debug)]
pub enum HostStreamOwnedWrite {
    Unix(tokio::net::unix::OwnedWriteHalf),
    Tcp(tokio::net::tcp::OwnedWriteHalf),
}

impl HostStreamInner {
    pub fn into_split(self) -> (HostStreamOwnedRead, HostStreamOwnedWrite) {
        match self {
            Self::Unix(stream) => {
                let (read, write) = stream.into_split();
                (
                    HostStreamOwnedRead::Unix(read),
                    HostStreamOwnedWrite::Unix(write),
                )
            }
            Self::Tcp(stream) => {
                let (read, write) = stream.into_split();
                (
                    HostStreamOwnedRead::Tcp(read),
                    HostStreamOwnedWrite::Tcp(write),
                )
            }
        }
    }
}

impl From<tokio::net::UnixStream> for HostStreamInner {
    fn from(value: tokio::net::UnixStream) -> Self {
        Self::Unix(value)
    }
}

impl From<tokio::net::TcpStream> for HostStreamInner {
    fn from(value: tokio::net::TcpStream) -> Self {
        Self::Tcp(value)
    }
}

impl tokio::io::AsyncRead for HostStreamOwnedRead {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Unix(stream) => Pin::new(stream).poll_read(cx, buf),
            Self::Tcp(stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for HostStreamOwnedWrite {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        match &mut *self {
            Self::Unix(stream) => Pin::new(stream).poll_write(cx, buf),
            Self::Tcp(stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Unix(stream) => Pin::new(stream).poll_flush(cx),
            Self::Tcp(stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Self::Unix(stream) => Pin::new(stream).poll_shutdown(cx),
            Self::Tcp(stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}
pub type HostStream = HostStreamInner;
pub type HostStreamReadHalf = HostStreamOwnedRead;
pub type HostStreamWriteHalf = HostStreamOwnedWrite;

#[cfg(unix)]
pub enum HostListener {
    Unix(tokio::net::UnixListener),
}

#[cfg(not(unix))]
pub enum HostListener {
    Loopback(tokio::net::TcpListener),
}

pub struct BoundEndpoint {
    #[allow(dead_code)]
    pub transport: crate::daemon::protocol::KernelTransport,
    #[cfg(unix)]
    pub socket_path: std::path::PathBuf,
    #[cfg(not(unix))]
    pub address: String,
}

pub async fn connect_endpoint() -> Result<HostStream, Error> {
    #[cfg(unix)]
    {
        let path = crate::daemon::protocol::default_socket_path();
        tokio::net::UnixStream::connect(&path)
            .await
            .map_err(|error| {
                Error::new(
                    FailureCode::ConnectionRefused,
                    format!(
                        "failed to connect to KernelHost {}: {error}",
                        path.display()
                    ),
                )
            })
            .map(Into::into)
    }
    #[cfg(not(unix))]
    {
        let address = crate::daemon::protocol::default_endpoint_address();
        tokio::net::TcpStream::connect(&address)
            .await
            .map_err(|error| {
                Error::new(
                    FailureCode::ConnectionRefused,
                    format!("failed to connect to KernelHost {address}: {error}"),
                )
            })
    }
}

pub async fn connect_tcp_endpoint(address: &str) -> Result<HostStream, Error> {
    TcpStream::connect(address)
        .await
        .map_err(|error| {
            Error::new(
                FailureCode::ConnectionRefused,
                format!("failed to connect to KernelHost {address}: {error}"),
            )
        })
        .map(Into::into)
}

impl HostListener {
    #[cfg(unix)]
    pub fn bind() -> Result<(Self, BoundEndpoint), Error> {
        let path = crate::daemon::protocol::default_socket_path();
        let listener = tokio::net::UnixListener::bind(&path).map_err(|error| {
            Error::new(
                FailureCode::ServiceUnavailable,
                format!(
                    "failed to bind KernelHost socket {}: {error}",
                    path.display()
                ),
            )
        })?;
        Ok((
            Self::Unix(listener),
            BoundEndpoint {
                transport: crate::daemon::protocol::KernelTransport::UnixSocket,
                socket_path: path,
            },
        ))
    }

    #[cfg(not(unix))]
    pub fn bind() -> Result<(Self, BoundEndpoint), Error> {
        let address = crate::daemon::protocol::default_endpoint_address();
        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .map_err(|error| {
                Error::new(
                    FailureCode::ServiceUnavailable,
                    format!("failed to bind KernelHost endpoint {address}: {error}"),
                )
            })?;
        let address = listener
            .local_addr()
            .map(|address| address.to_string())
            .map_err(|error| {
                Error::new(
                    FailureCode::ServiceUnavailable,
                    format!("failed to resolve KernelHost endpoint: {error}"),
                )
            })?;
        Ok((
            Self::Loopback(listener),
            BoundEndpoint {
                transport: crate::daemon::protocol::KernelTransport::LoopbackTcp,
                address,
            },
        ))
    }

    pub async fn accept(&self) -> Result<HostStream, Error> {
        match self {
            #[cfg(unix)]
            Self::Unix(listener) => match listener.accept().await {
                Ok((stream, _)) => Ok(stream.into()),
                Err(error) => Err(Error::new(
                    FailureCode::ServiceUnavailable,
                    format!("KernelHost accept failed: {error}"),
                )),
            },
            #[cfg(not(unix))]
            Self::Loopback(listener) => match listener.accept().await {
                Ok((stream, _)) => Ok(stream.into()),
                Err(error) => Err(Error::new(
                    FailureCode::ServiceUnavailable,
                    format!("KernelHost accept failed: {error}"),
                )),
            },
        }
    }
}

pub async fn write_line<W>(writer: &mut W, line: &str) -> Result<(), Error>
where
    W: AsyncWrite + Unpin,
{
    writer
        .write_all(format!("{line}\n").as_bytes())
        .await
        .map_err(|error| {
            Error::new(
                FailureCode::ConnectionClosed,
                format!("KernelHost write failed: {error}"),
            )
        })
}

pub use tokio::io::AsyncBufReadExt;
pub use tokio::io::AsyncWriteExt;
pub use tokio::io::BufReader;

pub async fn read_line<R>(reader: &mut BufReader<R>) -> Result<String, Error>
where
    R: AsyncRead + Unpin,
{
    let mut line = String::new();
    let bytes = reader.read_line(&mut line).await.map_err(|error| {
        Error::new(
            FailureCode::ConnectionClosed,
            format!("KernelHost read failed: {error}"),
        )
    })?;
    if bytes == 0 {
        return Err(Error::new(
            FailureCode::ConnectionClosed,
            "KernelHost connection closed",
        ));
    }
    Ok(line)
}
