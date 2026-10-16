use solana_client::rpc_response::transaction::CompiledInstruction;
use solana_sdk::signature::Signature;

pub mod rpc_fetcher;

/**
 * An already parsed Solana transaction
 */
pub struct RawTx {
    instructions: Vec<CompiledInstruction>
}

/**
 * An abstraction on top of a client that fetches transaction from Solana's blockchain
 */
// TODO: Does this need #[async_trait]? Check rustbook
trait Fetcher {
    async fn get_transaction(&self, signature: &Signature) -> Result<RawTx, anyhow::Error>;
}


