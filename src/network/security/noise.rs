use futures::ready;
use snow::{Builder, TransportState};
use std::{io::ErrorKind, pin::Pin, task::Poll};
use tokio::io::{self, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

use crate::{ClientSecurityStream, KvError, ServerSecurityStream};

// TODO(Wiccy): Support multi pattern
static PATTERN: &'static str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";

// 目前仅支持 Noise 下的 NN 方式，因此还不需要存储数据
pub struct NoiseInitiator;
pub struct NoiseResponder;

impl NoiseInitiator {
    pub fn new() -> Self {
        Self
    }
}
impl NoiseResponder {
    pub fn new() -> Self {
        Self
    }
}

pub struct ClientNoiseStream<S> {
    stream: S,
    initiator: TransportState,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}
pub struct ServerNoiseStream<S> {
    stream: S,
    responder: TransportState,
    read_buf: Vec<u8>,
    write_buf: Vec<u8>,
}

impl ClientSecurityStream for NoiseInitiator {
    type Stream<S> = ClientNoiseStream<S>;

    async fn connect<S>(&self, mut stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin,
    {
        let mut initiator = Builder::new(PATTERN.parse()?).build_initiator()?;

        // Noise handshake
        let mut first_msg = [0u8; 65535];
        let len = initiator.write_message(&[], &mut first_msg)?;
        stream.write_all(&first_msg[..len]).await?;
        let len = stream.read(&mut first_msg).await?;
        let mut read_buf = [0u8; 65535];
        initiator.read_message(&first_msg[..len], &mut read_buf)?;

        Ok(ClientNoiseStream {
            stream,
            initiator: initiator.into_transport_mode()?,
            read_buf: Vec::new(),
            write_buf: Vec::new(),
        })
    }
}

impl ServerSecurityStream for NoiseResponder {
    type Stream<S> = ServerNoiseStream<S>;

    async fn accept<S>(&self, mut stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin,
    {
        let mut responder = Builder::new(PATTERN.parse()?).build_responder()?;

        // Noise handshake
        let mut first_msg = [0u8; 65535];
        let len = stream.read(&mut first_msg).await?;
        let mut read_buf = [0u8; 65535];
        responder.read_message(&first_msg[..len], &mut read_buf)?;
        let len = responder.write_message(&[], &mut first_msg)?;
        stream.write_all(&first_msg[..len]).await?;

        Ok(ServerNoiseStream {
            stream,
            responder: responder.into_transport_mode()?,
            read_buf: Vec::new(),
            write_buf: Vec::new(),
        })
    }
}

impl<S: Unpin + AsyncRead> AsyncRead for ClientNoiseStream<S> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        if self.read_buf.is_empty() {
            let mut temp_buf = vec![0u8; 4096];
            let mut temp_read_buf = ReadBuf::new(&mut temp_buf);

            ready!(Pin::new(&mut self.stream).poll_read(cx, &mut temp_read_buf))?;
            let n = temp_read_buf.filled().len();

            let mut decrypted_buf = vec![0u8; n + 16];
            let len = self
                .initiator
                .read_message(&temp_buf[..n], &mut decrypted_buf)
                .map_err(|_| io::Error::new(ErrorKind::Other, "Decryption error"))?;

            self.read_buf.extend_from_slice(&decrypted_buf[..len]);
        }

        let len = std::cmp::min(buf.remaining(), self.read_buf.len());
        buf.put_slice(&self.read_buf[..len]);
        self.read_buf.drain(..len);

        Poll::Ready(Ok(()))
    }
}

impl<S: Unpin + AsyncWrite> AsyncWrite for ClientNoiseStream<S> {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();
        if this.write_buf.is_empty() {
            this.write_buf.resize(buf.len() + 16, 0);
            let len = this
                .initiator
                .write_message(buf, &mut this.write_buf)
                .map_err(|_| io::Error::new(ErrorKind::Other, "Encryption error"))?;
            this.write_buf.truncate(len);
        }

        let n = ready!(Pin::new(&mut this.stream).poll_write(cx, &this.write_buf))?;
        if n == 0 {
            return Poll::Ready(Err(io::Error::new(
                ErrorKind::WriteZero,
                "write zero bytes",
            )));
        }

        this.write_buf.drain(..n);

        if this.write_buf.is_empty() {
            Poll::Ready(Ok(buf.len()))
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        let stream = &mut this.stream;
        Pin::new(stream).poll_flush(cx)
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        let this = self.get_mut();
        let stream = &mut this.stream;
        Pin::new(stream).poll_shutdown(cx)
    }
}

impl<S: Unpin + AsyncRead> AsyncRead for ServerNoiseStream<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        // If the internal read buffer is empty, read from the stream.
        if self.read_buf.is_empty() {
            let mut temp_buf = vec![0u8; 4096];
            let mut temp_read_buf = ReadBuf::new(&mut temp_buf);

            // Poll the underlying stream to read data into temp_buf.
            ready!(Pin::new(&mut self.stream).poll_read(cx, &mut temp_read_buf))?;

            // Get the number of bytes read into temp_buf.
            let n = temp_read_buf.filled().len();

            // Decrypt the data and fill the internal read buffer.
            let mut decrypted_buf = vec![0u8; n + 16]; // Ensure enough space for decrypted data.
            let len = self
                .responder
                .read_message(&temp_buf[..n], &mut decrypted_buf)
                .map_err(|_| io::Error::new(ErrorKind::Other, "Decryption error"))?;

            self.read_buf.extend_from_slice(&decrypted_buf[..len]);
        }

        // Copy data from the internal read buffer to the provided buffer.
        let len = std::cmp::min(buf.remaining(), self.read_buf.len());
        buf.put_slice(&self.read_buf[..len]);
        self.read_buf.drain(..len);

        Poll::Ready(Ok(()))
    }
}

impl<S: Unpin + AsyncWrite> AsyncWrite for ServerNoiseStream<S> {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, std::io::Error>> {
        let this = self.get_mut();

        // 加密数据并填充到内部写入缓冲区。
        this.write_buf.clear();
        this.write_buf.resize(buf.len() + 16, 0); // 确保有足够的空间存放加密数据。
        let len = this
            .responder
            .write_message(buf, &mut this.write_buf)
            .map_err(|_| io::Error::new(ErrorKind::Other, "Encryption error"))?;

        // 从内部写入缓冲区写入数据到stream。
        let mut written = 0;
        while written < len {
            let n =
                ready!(Pin::new(&mut this.stream).poll_write(cx, &this.write_buf[written..len]))?;
            if n == 0 {
                return Poll::Ready(Err(io::Error::new(
                    ErrorKind::WriteZero,
                    "write zero bytes",
                )));
            }
            written += n;
        }

        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        Pin::new(&mut self.stream).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use anyhow::Result;
    use tokio::net::{TcpListener, TcpStream};

    use super::*;

    #[tokio::test]
    async fn noise_should_work() -> Result<()> {
        let addr = start_server().await?;

        let stream = TcpStream::connect(addr).await?;
        let mut stream = NoiseInitiator::connect(&NoiseInitiator::new(), stream).await?;
        stream.write_all(b"hello world!").await?;
        let mut buf = [0; 12];
        stream.read_exact(&mut buf).await?;
        assert_eq!(&buf, b"hello world!");

        Ok(())
    }

    async fn start_server() -> Result<SocketAddr> {
        let echo = TcpListener::bind("127.0.0.1:0").await?;
        let addr = echo.local_addr().unwrap();

        tokio::spawn(async move {
            let (stream, _) = echo.accept().await.unwrap();
            if let Ok(mut stream) = NoiseResponder::accept(&NoiseResponder::new(), stream).await {
                let mut buf = [0; 12];
                stream.read_exact(&mut buf).await.unwrap();
                stream.write_all(&buf).await.unwrap();
            }
        });

        Ok(addr)
    }
}
