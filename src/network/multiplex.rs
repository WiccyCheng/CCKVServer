use std::{future, marker::PhantomData, sync::Arc};

use futures::{stream, Future, TryStreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, Mutex},
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use yamux::{Config, Connection, ConnectionError, Mode};

// Yamux 控制结构
pub struct YamuxBuilder<S> {
    // sender 目前仅用于发送创建新的子流
    sender: mpsc::Sender<oneshot::Sender<Compat<yamux::Stream>>>,
    _s: PhantomData<S>,
}

impl<S> YamuxBuilder<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// 创建 Yamux 客户端，
    pub fn new_client(stream: S, config: Option<Config>) -> Self {
        Self::new(stream, config, true, |_stream| future::ready(Ok(())))
    }

    // 创建 YamuxCtrl 服务端，服务端 loop 在处理 substream
    pub fn new_server<F, Fut>(stream: S, config: Option<Config>, f: F) -> Self
    where
        F: FnMut(yamux::Stream) -> Fut,
        F: Send + 'static,
        Fut: Future<Output = Result<(), ConnectionError>> + Send + 'static,
    {
        Self::new(stream, config, false, f)
    }

    fn new<F, Fut>(stream: S, config: Option<Config>, is_client: bool, mut f: F) -> Self
    where
        F: FnMut(yamux::Stream) -> Fut,
        F: Send + 'static,
        Fut: Future<Output = Result<(), ConnectionError>> + Send + 'static,
    {
        let mode = if is_client {
            Mode::Client
        } else {
            Mode::Server
        };

        // 创建 Config
        let config = config.unwrap_or_default();

        // yamux::Stream 使用的是 futures 的 trait 所以需要 compat() 到 tokio 的 trait
        let conn = Connection::new(stream.compat(), config, mode);

        let conn = Arc::new(Mutex::new(conn));

        let (tx, mut rx) = mpsc::channel::<oneshot::Sender<Compat<yamux::Stream>>>(32);
        let conn_cloned = conn.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(sender) = rx.recv() => {
                        let mut conn = conn_cloned.lock().await;
                        // TODO(Wiccy): if ask for creating new substream before connection is fully initialzied，panic
                        // time::sleep(Duration::from_millis(100)).await;
                        let stream = future::poll_fn(|cx| conn.poll_new_outbound(cx)).await.expect("connection is probably not initialized yet");
                        let _ = sender.send(stream.compat());
                    }
                    // pull 所有 子流 的数据
                    _ = async {
                        let mut conn = conn_cloned.lock().await;
                        // 1. poll_fn() 会根据入参创建一个流，在这里为 调用Connection::poll_next_inbound()获取当前需要处理的子流
                        // 2. 1.产生的流我们称之为子流集， 那么接下来就会执行子流集.try_for_each_concurrent()
                        //    try_for_each_concurrent()为子流集 产生的每个子流异步执行 f
                        // 3.子流集 既然是个 stream ，在不主动调用 poll_next() 的情况下，是怎么完成的？ 答案在于 tokio,
                        //    这里 tokio 会不断调用 poll_next() 并轮询每个 substream
                        stream::poll_fn(|cx| conn.poll_next_inbound(cx))
                            .try_for_each_concurrent(None, |stream| {
                                let f = f(stream);
                                f
                            })
                            // 调用 await 来等待所有子流处理完毕，否则则会返回 Future，需要我们自己再处理 Future
                            .await
                    } => {}
                }
            }
        });

        Self {
            sender: tx,
            _s: Default::default(),
        }
    }

    // 打开一个新的 substream
    pub async fn open_stream(&mut self) -> Result<Compat<yamux::Stream>, ConnectionError> {
        // future::poll_fn() 与 上面的 stream::poll_fn()不同，它产生一个 Future 而不是 Stream
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(tx).await;
        rx.await.map_err(|_| ConnectionError::Closed)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        assert_res_ok,
        tls_utils::{tls_acceptor, tls_connector},
        utils::DummyStream,
        CommandRequest, KvError, MemTable, ProstClientStream, ProstServerStream, Service,
        ServiceInner, Storage, TlsServerAcceptor,
    };
    use anyhow::Result;
    use std::net::SocketAddr;
    use tokio::net::{TcpListener, TcpStream};
    use tokio_rustls::server;
    use tracing::warn;

    use super::*;

    #[tokio::test]
    async fn yamux_creation_should_work() -> Result<()> {
        let s = DummyStream::default();
        let mut client = YamuxBuilder::new_client(s, None);
        let stream = client.open_stream().await;

        assert!(stream.is_ok());
        Ok(())
    }

    #[tokio::test]
    async fn yamux_client_server_should_work() -> Result<()> {
        // 创建使用 TLS 的 yamux server
        let acceptor = tls_acceptor(false)?;
        let addr = start_yamux_server("127.0.0.1:0", acceptor, MemTable::new()).await?;

        let connector = tls_connector(false)?;
        let stream = TcpStream::connect(addr).await?;
        let stream = connector.connect(stream).await?;
        // 创建使用 TLS 的 yamux client
        let mut client = YamuxBuilder::new_client(stream, None);

        // 从 client 中打开一个新的 substream
        let stream = client.open_stream().await?;
        // 封装成 ProstClientStream
        let mut client = ProstClientStream::new(stream);

        let cmd = CommandRequest::new_hset("table", "key", "value");
        client.execute_unary(&cmd).await.unwrap();

        let cmd = CommandRequest::new_hget("table", "key");
        let res = client.execute_unary(&cmd).await.unwrap();
        assert_res_ok(&res, &["value".into()], &[]);

        Ok(())
    }

    pub async fn start_server_with<Store>(
        addr: &str,
        tls: TlsServerAcceptor,
        store: Store,
        f: impl Fn(server::TlsStream<TcpStream>, Service) + Send + Sync + 'static,
    ) -> Result<SocketAddr, KvError>
    where
        Store: Storage,
        Service: From<ServiceInner<Store>>,
    {
        let listener = TcpListener::bind(addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let service: Service = ServiceInner::new(store).into();

        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _addr)) => match tls.accept(stream).await {
                        Ok(stream) => f(stream, service.clone()),
                        Err(e) => warn!("Failed to process secure stream: {e:?}"),
                    },
                    Err(e) => warn!("Failed to process tcp {e:?}"),
                }
            }
        });

        Ok(addr)
    }

    /// 创建 yamux server
    pub async fn start_yamux_server<Store>(
        addr: &str,
        tls: TlsServerAcceptor,
        store: Store,
    ) -> Result<SocketAddr, KvError>
    where
        Store: Storage,
        Service: From<ServiceInner<Store>>,
    {
        let f = |stream, service: Service| {
            YamuxBuilder::new_server(stream, None, move |s| {
                let svc = service.clone();
                async move {
                    let stream = ProstServerStream::new(s.compat(), svc);
                    stream.process().await.unwrap();
                    Ok(())
                }
            });
        };
        start_server_with(addr, tls, store, f).await
    }
}
