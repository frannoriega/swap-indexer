use std::collections::HashMap;
use std::sync::LazyLock;

use anyhow::anyhow;
use clap::{Arg, Command, ValueEnum, value_parser};
use indexer::IndexerBuilder;
use indexer::accounts::{StaticTokenResolver, TokenInfo, token_accounts};
use indexer::interpreter::{Event, Interpreter};
use trade_parser_raydium::RaydiumParser;
use trade_parser_spl_token::SplTokenParser;
use rpc_fetcher::RpcFetcher;
use solana_sdk::pubkey;
use solana_sdk::{pubkey::Pubkey, signature::Signature};
use trade_interpreter::TradeInterpreter;
use trade_interpreter::event::summarize;
use trade_interpreter::resolver::resolve;
use types::Fetcher;

#[derive(Copy, Clone, ValueEnum)]
enum FetcherOpt {
    #[value(alias = "rpc")]
    Rpc,
}

const SOL_TOKEN: Pubkey = pubkey!("So11111111111111111111111111111111111111112");
const WSOL_TOKEN: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const SPL_TOKEN_2022: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const USDC_TOKEN: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
const RAYDIUM_AMM_V4: Pubkey = pubkey!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");

static KNOWN_TOKENS: LazyLock<HashMap<Pubkey, TokenInfo>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert(
        SOL_TOKEN,
        TokenInfo {
            symbol: "SOL".to_owned(),
            name: Some("Wrapped SOL".to_owned()),
        },
    );
    map.insert(
        WSOL_TOKEN,
        TokenInfo {
            symbol: "SOL".to_owned(),
            name: Some("Wrapped SOL".to_owned()),
        },
    );
    map.insert(
        SPL_TOKEN_2022,
        TokenInfo {
            symbol: "SOL".to_owned(),
            name: Some("Wrapped SOL".to_owned()),
        },
    );
    map.insert(
        USDC_TOKEN,
        TokenInfo {
            symbol: "USDC".to_owned(),
            name: Some("USDC".to_owned()),
        },
    );
    map
});

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
    let resolver = StaticTokenResolver::new(KNOWN_TOKENS.clone());
    let accounts = token_accounts(&tx);
    for (idx, trade) in TradeInterpreter::interpret(&processed.flow_graph)
        .iter()
        .enumerate()
        // TODO: Log dropped trades
        .filter_map(|(idx, trade)| {
            summarize(trade, &processed.call_tree, &accounts).map(|e| (idx, resolve(&e, &resolver)))
        })
    {
        println!("Trade {}: {}", idx + 1, trade.describe());
    }
    Ok(())
}
