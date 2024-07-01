use std::{marker::PhantomData, pin::Pin, task::Poll};

use bytes::BytesMut;
use futures::{ready, FutureExt, Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::{network::frame::read_frame, FrameCoder, KvError};

// 处理 KV server prost frame 的 stream
pub struct ProstStream<S, In, Out> {
    // inner stream
    stream: S,
    // 写缓存
    wbuf: BytesMut,
    // 写入了多少字节
    written: usize,
    // 读缓存
    rbuf: BytesMut,

    _in: PhantomData<In>,
    _out: PhantomData<Out>,
}

impl<S, In, Out> ProstStream<S, In, Out>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub fn new(stream: S) -> Self {
        Self {
            stream,
            written: 0,
            wbuf: BytesMut::new(),
            rbuf: BytesMut::new(),
            _in: PhantomData::default(),
            _out: PhantomData::default(),
        }
    }
}

impl<S, Req, Res> Unpin for ProstStream<S, Req, Res> where S: Unpin {}

impl<S, In, Out> Stream for ProstStream<S, In, Out>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
    In: Unpin + Send + FrameCoder,
    Out: Unpin + Send,
{
    // 调用 next() ，得到 Result<In, KvError>
    type Item = Result<In, KvError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // 上次调用结束后 rbuf 应该为空
        assert!(self.rbuf.is_empty());

        // 从 rbuf 中分理出 rest （摆脱对 self 的引用）
        let mut rest = self.rbuf.split_off(0);

        // 使用 read_frame 来读取数据
        let fut = read_frame(&mut self.stream, &mut rest);
        ready!(Box::pin(fut).poll_unpin(cx))?;

        // 拿到一个 frame 的数据后，把 buffer 合并回去
        self.rbuf.unsplit(rest);

        // 调用 decode_frame 获取解包后的数据
        Poll::Ready(Some(In::decode_frame(&mut self.rbuf)))
    }
}

// 当调用 send() 时，把 Out 发送出去
impl<S, In, Out> Sink<&Out> for ProstStream<S, In, Out>
where
    S: AsyncRead + AsyncWrite + Unpin,
    In: Unpin + Send,
    Out: Unpin + Send + FrameCoder,
{
    type Error = KvError;

    fn poll_ready(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        // 暂时不考虑在网络层做背压处理，依赖操作系统层的协议栈做背压处理
        Poll::Ready(Ok(()))
    }

    fn start_send(self: std::pin::Pin<&mut Self>, item: &Out) -> Result<(), Self::Error> {
        let this = self.get_mut();
        item.encode_frame(&mut this.wbuf)?;

        Ok(())
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        let this = self.get_mut();

        // 循环写入 stream 中
        while this.written != this.wbuf.len() {
            let n = ready!(Pin::new(&mut this.stream).poll_write(cx, &this.wbuf[this.written..]))?;
            this.written += n;
        }

        // 清除 buf
        this.wbuf.clear();
        this.written = 0;

        // 调用 stream 的 poll_flush 确保写入
        ready!(Pin::new(&mut this.stream).poll_flush(cx))?;
        Poll::Ready(Ok(()))
    }

    fn poll_close(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        // 调用 stream 的 poll_flush 确保写入
        ready!(self.as_mut().poll_flush(cx))?;

        // 调用 stream 的 poll_shutdown 确保 stream 关闭
        ready!(Pin::new(&mut self.stream).poll_shutdown(cx))?;
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{utils::DummyStream, CommandRequest};
    use anyhow::Result;
    use futures::prelude::*;

    #[tokio::test]
    async fn prost_stream_should_work() -> Result<()> {
        let stream = DummyStream::default();
        let mut stream = ProstStream::<_, CommandRequest, CommandRequest>::new(stream);
        let cmd = CommandRequest::new_hdel("table", "key");
        stream.send(&cmd).await?;
        if let Some(Ok(s)) = stream.next().await {
            assert_eq!(s, cmd)
        } else {
            assert!(false)
        }
        Ok(())
    }
}
