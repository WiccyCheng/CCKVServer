use std::io::{Read, Write};

use bytes::BufMut;
use lz4::{Decoder, EncoderBuilder};

use crate::{Compressor, KvError};

pub struct Lz4;
impl Compressor for Lz4 {
    fn compress(src: &[u8], dst: &mut bytes::BytesMut) -> Result<(), KvError> {
        let mut encoder = EncoderBuilder::new().build(dst.writer())?;
        encoder.write_all(src)?;
        let _ = encoder.finish();
        Ok(())
    }

    fn decompress(src: &[u8], dst: &mut Vec<u8>) -> Result<(), KvError> {
        let mut decoder = Decoder::new(src)?;
        decoder.read_to_end(dst)?;
        Ok(())
    }
}
