use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::UiTransactionEncoding};
use solana_sdk::signature::Signature;

use crate::{Fetcher, RawTx};

/**
 * [Fetcher] implementation using Solana's [RpcClient]
 */
pub struct RpcFetcher {
    client: RpcClient
}

impl RpcFetcher {
    pub fn new(url: String) -> RpcFetcher {
        RpcFetcher { client: RpcClient::new(url) }
    }
}

impl Fetcher for RpcFetcher {
    async fn get_transaction(&self, signature: &Signature) -> Result<RawTx, anyhow::Error> {
        let tx = self.client.get_transaction(signature, UiTransactionEncoding::JsonParsed)
            .await
            .map_err(|_| anyhow::Error::msg("error retrieving transaction"))?
            .transaction
            .transaction
            .decode()
            .ok_or(anyhow::Error::msg("could not decode transaction"))?
            .message;
        Ok(RawTx {
            instructions: tx.instructions().to_vec()
        })
    }
}
