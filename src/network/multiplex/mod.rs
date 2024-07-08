mod quic;
mod yamux;

pub use quic::*;
pub use yamux::*;

use crate::{KvError, ProstClientStream};

pub trait AppStream {
    type InnerStream;
    #[allow(async_fn_in_trait)]
    async fn open_stream(&mut self) -> Result<ProstClientStream<Self::InnerStream>, KvError>;
}
