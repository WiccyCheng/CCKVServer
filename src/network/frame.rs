use bytes::{Buf, BufMut, BytesMut};
use prost::Message;
use tokio::io::{AsyncRead, AsyncReadExt};
use tracing::debug;

use crate::{compress, decompress, CommandRequest, CommandResponse, CompressorType, KvError};

/// Frame头的长度占 4 个字节
const LEN_LEN: usize = 4;
/// 长度占30 bit，所以最大的 Frame 是 1G
const MAX_FRAME: usize = 1024 * 1024 * 1024;
/// 如果 payload 长度超过 1436 字节，就做压缩。
/// 以太网的 MTU 是 1500 字节，IP头、TCP头各占20字节，再除去IP头和TCP头可能包含的一些Option，我们预留 20 字节
/// 还剩 1440 字节，再减去预留的 4 字节做帧长度。超过 1436 字节可能会导致分片，所以我们做压缩处理
const COMPRESSION_LIMIT: usize = 1436;
/// 代表压缩的 bit 的位置整个长度为4字节的最高位）
const COMPRESSION_BIT: usize = 30;
/// 用于消除最高2位的掩码
const COMPRESSION_MASK: usize = 0x3FFFFFFF;

// 处理 Frame 的 encode/decode
pub trait FrameCoder
where
    Self: Message + Sized + Default,
{
    fn encode_frame(&self, buf: &mut BytesMut) -> Result<(), KvError> {
        self.encode_frame_with_compressor(buf, CompressorType::GZIP)
    }

    // 把一个 Message encode 成一个 Frame
    fn encode_frame_with_compressor(
        &self,
        buf: &mut BytesMut,
        compressor_type: CompressorType,
    ) -> Result<(), KvError> {
        let size = self.encoded_len();

        if size >= MAX_FRAME {
            return Err(KvError::FrameError);
        }

        // 先写入长度，如果需要压缩，再重写压缩后的长度
        buf.put_u32(size as _);

        if size > COMPRESSION_LIMIT {
            let mut buf_tmp = Vec::with_capacity(size);
            self.encode(&mut buf_tmp)?;

            // 为了 Frame 头部拿走 4 个字节
            let mut payload = buf.split_off(LEN_LEN);
            buf.clear();

            // 压缩
            compress(compressor_type, &buf_tmp[..], &mut payload)?;
            debug!("Encode a frame size: {size}({})", payload.len());

            // 写入压缩后的长度，同时把最高位置 1 表示该组数据经过压缩
            buf.put_u32((payload.len() | ((compressor_type as usize) << COMPRESSION_BIT)) as _);

            // 合并 BytesMut
            buf.unsplit(payload);

            Ok(())
        } else {
            self.encode(buf)?;
            Ok(())
        }
    }

    /// 把一个完整的 frame decode 成一个 Message
    fn decode_frame(buf: &mut BytesMut) -> Result<Self, KvError> {
        // 先取 4 字节，从中获得长度和 compression bit
        let header = buf.get_u32() as usize;
        let (len, compress_type) = decode_header(header);
        debug!("Got a frame: msg len: {len}, compress_type: {compress_type:?}");

        if compress_type != CompressorType::None {
            // 解压缩
            let mut buf_tmp = Vec::with_capacity(len * 2);
            decompress(compress_type, &buf[..len], &mut buf_tmp)?;
            buf.advance(len);

            Ok(Self::decode(&buf_tmp[..buf_tmp.len()])?)
        } else {
            let msg = Self::decode(&buf[..len])?;
            buf.advance(len);
            Ok(msg)
        }
    }
}

impl FrameCoder for CommandRequest {}
impl FrameCoder for CommandResponse {}

fn decode_header(header: usize) -> (usize, CompressorType) {
    let len = header & COMPRESSION_MASK;
    let compress_type: CompressorType = ((header & !COMPRESSION_MASK) >> COMPRESSION_BIT).into();
    (len, compress_type)
}

/// 从 stream 中读取一个完整的 frame
pub async fn read_frame<S>(stream: &mut S, buf: &mut BytesMut) -> Result<(), KvError>
where
    S: AsyncRead + Unpin + Send,
{
    let header = stream.read_u32().await? as usize;
    let (len, _compressed) = decode_header(header);
    // 确保内存至少可以放下一个 Frame。reserve()仅修改容量，即capacit()
    buf.reserve(LEN_LEN + len);
    buf.put_u32(header as _);
    // advance_mut 是 unsafe 的原因是，从当前位置 pos 到 pos + len，
    // 这段内存目前没有初始化。我们就是为了 reserve 这段内存，然后从 stream
    // 里读取，读取完，它就是初始化的。所以，我们这么用是安全的
    // 通过advance_mut()将buf的长度增加，即len()。上面已经reserve()了，所以容量是够的
    unsafe { buf.advance_mut(len) };
    stream.read_exact(&mut buf[LEN_LEN..]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::DummyStream;
    use crate::Value;
    use bytes::Bytes;

    #[tokio::test]
    async fn read_frame_should_work() {
        let mut buf = BytesMut::new();
        let cmd = CommandRequest::new_hdel("table", "key");
        cmd.encode_frame(&mut buf).unwrap();
        let mut stream = DummyStream { buf };

        let mut data = BytesMut::new();
        read_frame(&mut stream, &mut data).await.unwrap();

        let cmd_decoded = CommandRequest::decode_frame(&mut data).unwrap();
        assert_eq!(cmd, cmd_decoded);
    }

    #[test]
    fn command_request_encode_decode_should_work() {
        let mut buf = BytesMut::new();

        let cmd = CommandRequest::new_hdel("table", "key");
        cmd.encode_frame_with_compressor(&mut buf, CompressorType::LZ4)
            .unwrap();

        // 最高位未设置压缩标志
        assert_eq!(is_compressed(&buf), false);

        let cmd_decoded = CommandRequest::decode_frame(&mut buf).unwrap();
        assert_eq!(cmd, cmd_decoded);
    }

    #[test]
    fn command_response_encode_decode_should_work() {
        let mut buf = BytesMut::new();

        let values: Vec<Value> = vec![1.into(), "hello".into(), b"data".into()];
        let res: CommandResponse = values.into();
        res.encode_frame_with_compressor(&mut buf, CompressorType::ZSTD)
            .unwrap();

        // 最高位未设置压缩标志
        assert_eq!(is_compressed(&buf), false);

        let res_decoded = CommandResponse::decode_frame(&mut buf).unwrap();
        assert_eq!(res, res_decoded);
    }

    #[test]
    fn command_response_compressed_encode_decode_should_work() {
        let mut buf = BytesMut::new();

        let value: Value = Bytes::from(vec![0u8; COMPRESSION_LIMIT + 1]).into();
        let res: CommandResponse = value.into();
        res.encode_frame_with_compressor(&mut buf, CompressorType::GZIP)
            .unwrap();

        assert_eq!(is_compressed(&buf), true);

        let res_decoded = CommandResponse::decode_frame(&mut buf).unwrap();
        assert_eq!(res, res_decoded);
    }

    fn is_compressed(data: &[u8]) -> bool {
        if let &[v] = &data[..1] {
            v >> 6 != 0b00
        } else {
            false
        }
    }
}
