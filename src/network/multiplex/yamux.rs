use std::{future, marker::PhantomData, sync::Arc};

use futures::{stream, Future, TryStreamExt};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    sync::{mpsc, oneshot, Mutex},
};
use tokio_util::compat::{Compat, FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};
use tracing::instrument;
use yamux::{Config, Connection, ConnectionError, Mode};

use crate::{AppStream, KvError, ProstClientStream};

// Yamux 控制结构
pub struct YamuxConn<S> {
    // sender 目前仅用于发送创建新的子流
    sender: mpsc::Sender<oneshot::Sender<Compat<yamux::Stream>>>,
    _s: PhantomData<S>,
}

impl<S> YamuxConn<S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    /// 创建 Yamux 客户端，
    pub fn new_client(stream: S, config: Option<Config>) -> Self {
        Self::new(stream, config, true, |_stream| future::ready(Ok(())))
    }

    // 创建 Yamux 服务端，服务端 loop 在处理 substream
    pub fn new_server<F, Fut>(stream: S, config: Option<Config>, f: F) -> Self
    where
        F: FnMut(yamux::Stream) -> Fut,
        F: Send + 'static,
        Fut: Future<Output = Result<(), ConnectionError>> + Send + 'static,
    {
        Self::new(stream, config, false, f)
    }

    #[instrument(name = "yamux_builder_new", skip_all)]
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
                // 在 tokio::select! 中，每个分支的 Future 都会被逐一 poll，因此即使 poll_next_inbound 分支正在运行，只要 rx.recv() 分支准备好，
                // 它就会被选中执行，获取锁并创建新子流。 因为 tokio::select! 会取消未选中的分支的 Future，并在下一次轮询中重新 poll 它们，
                // 所以 poll_next_inbound 分支不会无限期占有锁。
                tokio::select! {
                    Some(sender) = rx.recv() => {
                        let mut conn = conn_cloned.lock().await;
                        // TODO(Wiccy): if ask for creating new substream before connection is fully initialzied，panic
                        // time::sleep(Duration::from_millis(100)).await;
                        let stream = future::poll_fn(|cx| conn.poll_new_outbound(cx)).await.expect("connection is probably not initialized yet");
                        let _ = sender.send(stream.compat());
                    }
                    _ = async { // 一直执行，要么在处理子流数据，要么在等待子流数据到来，除非 poll_next_inbound() 返回 None
                        let mut conn = conn_cloned.lock().await;
                        // 每个 Future 执行完都会释放锁， 所以在任意小 Future 挂起时或大 Future 取消时释放锁
                        // 在单调度器中，因为不会被 rx.recv() 分支抢占，所以永远不会挂起或取消
                        stream::poll_fn(|cx| conn.poll_next_inbound(cx))
                            .try_for_each_concurrent(None, |stream| {
                                let f = f(stream);
                                f
                            })
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
}

impl<S> AppStream for YamuxConn<S> {
    type InnerStream = Compat<yamux::Stream>;

    // 打开一个新的 substream
    #[instrument(skip_all)]
    async fn open_stream(&mut self) -> Result<ProstClientStream<Self::InnerStream>, KvError> {
        let (tx, rx) = oneshot::channel();
        let _ = self.sender.send(tx).await;
        Ok(ProstClientStream::new(
            rx.await.map_err(|_| ConnectionError::Closed)?,
        ))
    }
}
#[cfg(test)]
mod tests {
    use crate::{
        assert_res_ok,
        tls_utils::{tls_acceptor, tls_connector},
        utils::DummyStream,
        CommandRequest, KvError, MemTable, ProstServerStream, Service, ServiceInner, Storage,
        TlsServerAcceptor,
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
        let mut client = YamuxConn::new_client(s, None);
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
        let mut client = YamuxConn::new_client(stream, None);

        // 从 client 中打开一个新的 ProstClientStream substream
        let mut stream = client.open_stream().await?;

        let cmd = CommandRequest::new_hset("table", "key", "value");
        stream.execute_unary(&cmd).await.unwrap();

        let cmd = CommandRequest::new_hget("table", "key");
        let res = stream.execute_unary(&cmd).await.unwrap();
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
            YamuxConn::new_server(stream, None, move |s| {
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
