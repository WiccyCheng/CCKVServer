mod quic;
mod yamux;
pub use quic::*;
pub use yamux::*;

use crate::{KvError, ProstClientStream};
use std::future::Future;

pub trait AppStream {
    type InnerStream;
    fn open_stream(
        &mut self,
    ) -> impl Future<Output = Result<ProstClientStream<Self::InnerStream>, KvError>>;
}
