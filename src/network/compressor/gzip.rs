use std::io::{Read, Write};

use bytes::{BufMut, BytesMut};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};

use crate::{Compressor, KvError};

pub struct Gzip;
impl Compressor for Gzip {
    fn compress(src: &[u8], dst: &mut BytesMut) -> Result<(), KvError> {
        let mut encoder = GzEncoder::new(dst.writer(), Compression::default());
        encoder.write_all(&src[..])?;
        encoder.finish()?;
        Ok(())
    }

    fn decompress(src: &[u8], dst: &mut Vec<u8>) -> Result<(), KvError> {
        let mut decoder = GzDecoder::new(src);
        decoder.read_to_end(dst)?;
        Ok(())
    }
}
