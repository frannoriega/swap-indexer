use anyhow::anyhow;
use clap::{Arg, Command, ValueEnum, value_parser};
use indexer::IndexerBuilder;
use indexer::interpreter::Interpreter;
use indexer::interpreter::trade::TradeInterpreter;
use parser_core::system::SplTokenParser;
use parser_raydium::RaydiumParser;
use rpc_fetcher::RpcFetcher;
use solana_sdk::pubkey;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use types::Fetcher;

#[derive(Copy, Clone, ValueEnum)]
enum FetcherOpt {
    #[value(alias = "rpc")]
    Rpc,
}

const WSOL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let matches = Command::new("Swap indexer")
        .version("0.1.0")
        .arg(
            Arg::new("url")
                .required(true)
                .help("Set the URL for the node")
                .ignore_case(true),
        )
        .arg(
            Arg::new("signature")
                .required(true)
                .help("Transaction signature")
                .value_parser(value_parser!(Signature))
                .ignore_case(true),
        )
        .arg(
            Arg::new("fetcher")
                .required(true)
                .help("Choose the fetcher to use")
                .value_parser(value_parser!(FetcherOpt))
                .ignore_case(true),
        )
        .get_matches();

    let url = matches
        .get_one::<String>("url")
        .ok_or_else(|| anyhow!("URL not set"))?;
    let signature = matches
        .get_one::<Signature>("signature")
        .ok_or_else(|| anyhow!("signature not set"))?;
    let fetcher_opt = matches
        .get_one::<FetcherOpt>("fetcher")
        .ok_or_else(|| anyhow!("fetcher not set"))?;

    let fetcher = match fetcher_opt {
        FetcherOpt::Rpc => RpcFetcher::new(url.to_owned()),
    };
    let tx = fetcher.get_transaction(signature).await?;
    let indexer = IndexerBuilder::new()
        .with_parser(RAYDIUM_AMM_V4, RaydiumParser)
        .with_parser(WSOL_TOKEN, SplTokenParser)
        .with_parser(SPL_TOKEN_2022, SplTokenParser)
        .build();
    let processed = indexer.process(&tx)?;
    let _trades = TradeInterpreter::interpret(&processed.flow_graph);
    Ok(())
}
