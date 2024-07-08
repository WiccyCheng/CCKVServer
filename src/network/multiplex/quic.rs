use s2n_quic::{stream::BidirectionalStream, Connection};

use crate::{AppStream, ProstClientStream};

pub struct QuicConn {
    conn: Connection,
}

impl QuicConn {
    pub fn new(conn: Connection) -> Self {
        Self { conn }
    }
}

impl AppStream for QuicConn {
    type InnerStream = BidirectionalStream;

    async fn open_stream(
        &mut self,
    ) -> Result<crate::ProstClientStream<Self::InnerStream>, crate::KvError> {
        let stream = self.conn.open_bidirectional_stream().await?;
        Ok(ProstClientStream::new(stream))
    }
}

//TODO(Wiccy): can not produce a proper private key for quic server currently, so skip the test
// #[cfg(test)]
// mod tests {
//     use anyhow::Result;

//     use crate::{
//         start_quic_client_with_config, start_quic_server, ClientConfig, MemTable, ServerConfig,
//     };

//     use super::*;

//     #[tokio::test]
//     async fn quic_creation_should_work() -> Result<()> {
//         let server_config: ServerConfig =
//             toml::from_str(include_str!("../../../fixtures/quic_server.conf")).unwrap();
//         start_quic_server(
//             &server_config.general.addr,
//             MemTable::new(),
//             &server_config.security,
//         )
//         .await
//         .unwrap();

//         let client_config: ClientConfig =
//             toml::from_str(include_str!("../../../fixtures/quic_client.conf")).unwrap();

//         let mut client = start_quic_client_with_config(&client_config).await.unwrap();
//         let stream = client.open_stream().await;
//         assert!(stream.is_ok());

//         Ok(())
//     }
// }
