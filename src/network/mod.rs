mod compressor;
mod frame;
mod multiplex;
mod security;
mod stream;
mod stream_result;

pub use compressor::*;
pub use frame::FrameCoder;
pub use multiplex::*;
pub use security::*;
use stream::*;

use futures::{SinkExt, StreamExt};
use stream_result::StreamResult;
use tokio::io::{AsyncRead, AsyncWrite};
use tracing::info;

use crate::{CommandRequest, CommandResponse, KvError, Service, Storage};

// 处理服务端某个 accept 下来的 socket 的读写
pub struct ProstServerStream<S, Store> {
    inner: ProstStream<S, CommandRequest, CommandResponse>,
    service: Service<Store>,
}

// 处理客户端 socket 的读写
pub struct ProstClientStream<S> {
    inner: ProstStream<S, CommandResponse, CommandRequest>,
}

impl<S, Store> ProstServerStream<S, Store>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    Store: Storage,
{
    pub fn new(stream: S, service: Service<Store>) -> Self {
        Self {
            inner: ProstStream::new(stream),
            service,
        }
    }

    pub async fn process(mut self) -> Result<(), KvError> {
        let stream = &mut self.inner;
        while let Some(Ok(cmd)) = stream.next().await {
            info!("Got a new command: {cmd:?}");
            let mut res = self.service.execute(cmd);
            while let Some(data) = res.next().await {
                stream.send(&data).await?;
            }
        }
        Ok(())
    }
}

impl<S> ProstClientStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(stream: S) -> Self {
        Self {
            inner: ProstStream::new(stream),
        }
    }

    pub async fn execute_unary(
        &mut self,
        cmd: &CommandRequest,
    ) -> Result<CommandResponse, KvError> {
        let stream = &mut self.inner;
        stream.send(&cmd).await?;

        match stream.next().await {
            Some(v) => v,
            None => Err(KvError::Internal("Didn't get any response".into())),
        }
    }

    pub async fn execute_streaming(self, cmd: &CommandRequest) -> Result<StreamResult, KvError> {
        let mut stream = self.inner;

        stream.send(cmd).await?;
        stream.close().await?;

        StreamResult::new(stream).await
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use bytes::Bytes;
    use std::net::SocketAddr;

    use tokio::net::{TcpListener, TcpStream};

    use crate::{assert_res_ok, MemTable, ServiceInner, Value};

    use super::*;

    #[tokio::test]
    async fn client_server_basic_communication_should_work() -> anyhow::Result<()> {
        let addr = start_server().await?;

        let stream = TcpStream::connect(addr).await?;
        let mut client = ProstClientStream::new(stream);

        // 发送 HSET，等待回应
        let cmd = CommandRequest::new_hset("table", "key", "value");
        let res = client.execute_unary(&cmd).await.unwrap();

        // 第一次 HSET 服务器应该返回 None
        assert_res_ok(&res, &[Value::default()], &[]);

        // 再发一个 HSET
        let cmd = CommandRequest::new_hget("table", "key");
        let res = client.execute_unary(&cmd).await.unwrap();

        // 服务器应该返回上一次的结果
        assert_res_ok(&res, &["value".into()], &[]);

        // 发一个 SUBSCRIBE
        let cmd = CommandRequest::new_subscribe("chat");
        let res = client.execute_streaming(&cmd).await.unwrap();
        let id = res.id;
        assert!(id > 0);

        Ok(())
    }

    #[tokio::test]
    async fn client_server_compression_should_work() -> anyhow::Result<()> {
        let addr = start_server().await?;

        let stream = TcpStream::connect(addr).await?;
        let mut client = ProstClientStream::new(stream);

        let value: Value = Bytes::from(vec![0u8; 16384]).into();
        let cmd = CommandRequest::new_hset("table", "key", value.clone());
        let res = client.execute_unary(&cmd).await.unwrap();

        assert_res_ok(&res, &[Value::default()], &[]);

        let cmd = CommandRequest::new_hget("table", "key");
        let res = client.execute_unary(&cmd).await.unwrap();

        assert_res_ok(&res, &[value.into()], &[]);

        Ok(())
    }

    async fn start_server() -> Result<SocketAddr> {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            loop {
                let (stream, _) = listener.accept().await.unwrap();
                let service: Service = ServiceInner::new(MemTable::new()).into();
                let server = ProstServerStream::new(stream, service);
                tokio::spawn(server.process());
            }
        });

        Ok(addr)
    }
}

#[cfg(test)]
pub mod utils {
    use bytes::{BufMut, BytesMut};
    use std::{cmp::min, task::Poll};
    use tokio::io::{AsyncRead, AsyncWrite};

    #[derive(Default)]
    pub struct DummyStream {
        pub buf: BytesMut,
    }

    impl AsyncRead for DummyStream {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            // 看看 Reader 需要多大的数据
            let this = self.get_mut();
            let len = min(buf.capacity(), this.buf.len());

            // split 出这么大的数据
            let data = this.buf.split_to(len);

            // 拷贝给 ReadBuf
            buf.put_slice(&data);

            std::task::Poll::Ready(Ok(()))
        }
    }

    impl AsyncWrite for DummyStream {
        fn poll_write(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize, std::io::Error>> {
            self.get_mut().buf.put_slice(buf);
            Poll::Ready(Ok(buf.len()))
        }

        fn poll_flush(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(
            self: std::pin::Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> Poll<Result<(), std::io::Error>> {
            Poll::Ready(Ok(()))
        }
    }
}
