# swap-indexer

A small, honest Solana **swap indexer** for a single DEX program. It streams live
swap events off-chain over Yellowstone gRPC, decodes them with
[Carbon](https://github.com/sevenlabs-hq/carbon), persists them to Postgres, and
serves them over a tiny read-only HTTP API.

This is a scoped proof-of-concept meant to be read and understood quickly — not a
production indexer.

**Target program:** [Raydium AMM V4](https://solscan.io/account/675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8)
(`675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8`). Swapping to another DEX is a
localized change: replace the decoder crate and the match arms in
`ingestion/src/processor.rs`.

## What it does

- Subscribes to a Yellowstone gRPC endpoint, filtered to the Raydium AMM V4 program.
- Decodes `SwapBaseIn` / `SwapBaseOut` instructions via Carbon.
- Resolves the real mints and amounts from the transaction's token-balance deltas
  and upserts one row per swap, keyed by transaction signature.
- Serves two read queries over the stored swaps.

## What it does NOT do (v1 non-goals)

These are deliberately out of scope. The three-crate split is the seam that would
let them be added later without a rewrite.

- **Historical backfill / replay** from a given slot — live stream only.
- **Multi-program / multi-source** support — one program, one datasource.
- **Reorg handling** beyond upsert-by-signature idempotency. If a signature is
  re-delivered, the row is not duplicated; dropped/rolled-back slots are not undone.
- **Auth, rate limiting, multi-tenancy** — none of it.

## Architecture

Three separate crates in a Cargo workspace — kept separate even at this size so the
non-goals above stay addable:

| Crate        | Role                                                                    |
| ------------ | ----------------------------------------------------------------------- |
| `db`         | Shared `Swap` row type + pool helper + migrations (the schema).         |
| `ingestion`  | The **only writer**: Yellowstone gRPC → Carbon decode → upsert.         |
| `api`        | **Read-only** HTTP API (Axum + SQLx) over the same Postgres.            |

### Data model

One `swaps` table (see `db/migrations/0001_create_swaps.sql`):

- `signature` is `UNIQUE` and is the **idempotency key** — the pipeline upserts by
  it, so at-least-once delivery / reconnects never duplicate a swap.
- `mint_in` / `mint_out` preserve swap direction; `pair` is a canonical
  order-independent key (`min(mint)-max(mint)`) backing the `?pair=` query.
- Exactly two indexes, for the two query patterns: `wallet`, and `(pair, block_time)`.
  No speculative indexes.

## API

```
GET /swaps?wallet=<pubkey>
GET /swaps?pair=<mint_a>-<mint_b>&since=<unix_seconds>
```

- `wallet` — most recent swaps by that wallet.
- `pair` — swaps for a token pair; order of the two mints doesn't matter. `since`
  is optional Unix seconds (defaults to the epoch).
- Both return at most 100 rows, newest first, as JSON. Provide exactly one of
  `wallet` or `pair`.

## Running locally

Prerequisites: Rust, Docker, and a Yellowstone gRPC endpoint (free tier from
Triton / Helius / Shyft is fine).

```bash
# 1. Config
cp .env.example .env
#    then set YELLOWSTONE_GRPC_URL (and YELLOWSTONE_X_TOKEN if your provider needs it)

# 2. Postgres
docker compose up -d

# 3. Ingestion — connects, applies migrations, and starts writing swaps.
cargo run -p ingestion

# 4. API (in another terminal)
cargo run -p api

# 5. Query
curl 'http://localhost:3000/swaps?wallet=<pubkey>'
curl 'http://localhost:3000/swaps?pair=<mint_a>-<mint_b>&since=0'
```

Migrations are applied automatically by the ingestion service on startup (it owns
the schema). The API assumes the schema already exists.

## Working in this project (how to test your changes)

The awkward part of an indexer is that the ingestion path needs a live gRPC
stream, which you can't spin up locally. So testing is layered — most work is
verifiable *without* live data:

**1. Pure logic — unit tests (no services needed).**
The swap-mapping logic (leg resolution, pair key, price) lives in
`ingestion/src/mapping.rs`, decoupled from Carbon and Postgres. Test it directly:

```bash
cargo test -p ingestion        # mapping unit tests
cargo test --workspace         # everything
```

If you change how a swap becomes a row, add a case here — it's the fast feedback loop.

**2. DB + API — seed synthetic data, then curl (Postgres only).**
You don't need the stream to work on the API. Seed the table and hit the endpoints:

```bash
docker compose up -d
cargo run -p benchmarks --bin seed -- --rows 5000   # prints a wallet + pair to query
cargo run -p api
# in another terminal, use the wallet/pair the seeder printed:
curl 'http://localhost:3000/swaps?wallet=<printed-wallet>'
curl 'http://localhost:3000/swaps?pair=<mint_a>-<mint_b>&since=0'
```

Inspect the raw rows with the optional Adminer UI:
`docker compose --profile tools up -d` → <http://localhost:8080>.

**3. Full end-to-end — real stream (needs a Yellowstone endpoint).**
Set `YELLOWSTONE_GRPC_URL` in `.env`, run `cargo run -p ingestion`, watch swaps
log and land in the table (`RUST_LOG=info`), then query them through the API. This
is the only step that needs live infrastructure.

## Benchmarking

Two manually-triggered benchmarks cover the metrics that matter for an indexer:
per-swap processing cost, write throughput, and query latency.

**Processing throughput (CPU cost per swap, no DB):**

```bash
cargo bench -p ingestion
```

Criterion micro-benchmark over `build_swap_row` — the decode→row work done for
every swap. Report lands in `target/criterion/`.

**Write throughput + query latency (needs Postgres):**

```bash
docker compose up -d
cargo run -p benchmarks --release --bin loadtest -- --rows 100000 --iterations 1000
```

Seeds N rows (reporting rows/sec) then runs the exact SQL the API runs, reporting
**p50 / p90 / p99 / max** latency for both query patterns. Running the same SQL as
the API isolates data-layer performance (index + planner + fetch) from HTTP
overhead. Increase `--rows` to see how latency holds as the table grows — the two
indexes (`wallet`, `(pair, block_time)`) are what keep it flat.

> `loadtest` truncates the `swaps` table for a clean baseline; run it against a dev
> database. Pass `--no-truncate` to measure against existing data.

**Not covered offline:** end-to-end ingestion lag (block time → row committed)
needs a live stream to measure honestly; watch the pipeline's log metrics
(`RUST_LOG=info`) for that when running against a real endpoint.

## Notes on a few decisions

- **Upsert by signature** — the cheapest correct idempotency guarantee for an
  at-least-once stream, and the only reorg handling v1 attempts.
- **Amounts/mints from token-balance deltas** — the instruction data only carries
  requested/limit amounts and the AMM V4 account list doesn't carry mints, so the
  realized figures come from the wallet's pre/post token balances in the tx meta.
- **Runtime SQLx queries** (`query_as`) rather than the compile-time `query!`
  macros, so the workspace builds with no database present. To switch to
  compile-time-checked queries, run against a migrated DB with `DATABASE_URL` set
  (or `cargo sqlx prepare` for an offline cache) and swap `query_as` for `query_as!`.
