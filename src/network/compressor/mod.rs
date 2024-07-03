mod gzip;
mod lz4;
mod zstd;

use crate::KvError;
use bytes::BytesMut;
use gzip::*;
use lz4::*;
use zstd::*;

// 处理数据的压缩和解压
pub trait Compressor {
    fn compress(src: &[u8], dst: &mut BytesMut) -> Result<(), KvError>;
    fn decompress(src: &[u8], dst: &mut Vec<u8>) -> Result<(), KvError>;
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum CompressorType {
    None = 0,
    GZIP,
    LZ4,
    ZSTD,
}

pub fn compress(compressor: CompressorType, src: &[u8], dst: &mut BytesMut) -> Result<(), KvError> {
    match compressor {
        CompressorType::GZIP => Gzip::compress(src, dst),
        CompressorType::LZ4 => Lz4::compress(src, dst),
        CompressorType::ZSTD => Zstd::compress(src, dst),
        CompressorType::None => Ok(()),
    }
}

pub fn decompress(
    compressor: CompressorType,
    src: &[u8],
    dst: &mut Vec<u8>,
) -> Result<(), KvError> {
    match compressor {
        CompressorType::GZIP => Gzip::decompress(src, dst),
        CompressorType::LZ4 => Lz4::decompress(src, dst),
        CompressorType::ZSTD => Zstd::decompress(src, dst),
        CompressorType::None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    fn gzip_should_work() {
        compressor_should_work(CompressorType::GZIP);
    }

    #[test]
    fn lz4_should_work() {
        compressor_should_work(CompressorType::LZ4);
    }

    #[test]
    fn zstd_should_work() {
        compressor_should_work(CompressorType::ZSTD);
    }

    fn compressor_should_work(compressor_type: CompressorType) {
        let data = b"data that will be compressed.";
        let mut compressed = BytesMut::new();
        let mut decompressed = Vec::new();

        let res = compress(compressor_type, data, &mut compressed);
        assert!(res.is_ok());

        let _ = decompress(compressor_type, &compressed, &mut decompressed);
        assert_eq!(decompressed, data);
    }
}

impl From<usize> for CompressorType {
    fn from(value: usize) -> Self {
        match value {
            1 => CompressorType::GZIP,
            2 => CompressorType::LZ4,
            3 => CompressorType::ZSTD,
            _ => CompressorType::None,
        }
    }
}
