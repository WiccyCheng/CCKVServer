mod noise;
mod tls;
pub use noise::*;
pub use tls::*;

use tokio::io::{AsyncRead, AsyncWrite};

use crate::KvError;
pub trait ClientSecurityStream {
    // TODO(Wiccy): remove this after upgrading to 2024 edition
    #![allow(async_fn_in_trait)]
    type Stream<S>;

    async fn connect<S>(&self, stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin;
}

pub trait ServerSecurityStream {
    // TODO(Wiccy): remove this after upgrading to 2024 edition
    #![allow(async_fn_in_trait)]
    type Stream<S>;

    async fn accept<S>(&self, stream: S) -> Result<Self::Stream<S>, KvError>
    where
        S: AsyncRead + AsyncWrite + Send + Unpin;
}
