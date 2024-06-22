use zstd::{decode_all, encode_all};

use crate::{Compressor, KvError};

pub struct Zstd;
impl Compressor for Zstd {
    fn compress(src: &[u8], dst: &mut bytes::BytesMut) -> Result<(), KvError> {
        let compressed = encode_all(src, 0)?;
        dst.extend_from_slice(&compressed);
        Ok(())
    }

    fn decompress(src: &[u8], dst: &mut Vec<u8>) -> Result<(), KvError> {
        let decompressed = decode_all(src)?;
        dst.extend_from_slice(&decompressed);
        Ok(())
    }
}
