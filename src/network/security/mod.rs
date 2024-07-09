mod noise;
mod tls;
pub use noise::*;
pub use tls::*;

use std::future::Future;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::KvError;

pub trait SecureStreamConnect<S: AsyncRead + AsyncWrite + Send + Unpin> {
    type InnerStream: AsyncRead + AsyncWrite + Send + Unpin;
    fn connect(&self, stream: S) -> impl Future<Output = Result<Self::InnerStream, KvError>>;
}

pub trait SecureStreamAccept<S: AsyncRead + AsyncWrite + Send + Unpin> {
    type InnerStream: AsyncRead + AsyncWrite + Send + Unpin;
    fn accept(&self, stream: S) -> impl Future<Output = Result<Self::InnerStream, KvError>> + Send;
}
