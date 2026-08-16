# CLAUDE.md

Guidance for Claude Code working in this repo. This is a scoped POC, not a production indexer — the point is a complete, honest, working artifact to open-source, not maximal coverage.

## What this is

A Solana swap indexer for one DEX program, exposing a small read API. It streams live swap events off-chain, decodes them, persists them, and serves queries over them.

**Explicit non-goals for v1** (do not build these unless asked):
- Historical backfill / replay from a given slot
- Multi-program or multi-source support
- Reorg handling beyond upsert-by-signature idempotency
- Auth, rate limiting, or anything multi-tenant

If a task seems to require one of the above, stop and flag it rather than silently expanding scope.

## Architecture (three layers — keep them separate)

**Data layer (Postgres):**
- One `swaps` table: wallet, token pair (mint addresses, not symbols), amounts, price, slot, tx signature (unique — this is the idempotency key), block time.
- Index on `wallet` and on `(pair, block_time)` — those are the only two query patterns v1 needs. Don't add speculative indexes.

**Ingestion pipeline (Rust, async):**
- Yellowstone gRPC client → subscription filtered to the target program ID.
- Carbon (`sevenlabs-hq/carbon`) for the datasource → decoder → processor pipeline. Use an existing decoder crate for the target program if Carbon has one; otherwise generate one with `carbon-cli parse --idl` before hand-writing anything.
- Processor's only job: map a decoded swap instruction to a `swaps` row and upsert by tx signature. No business logic beyond that in v1.

**API layer (Axum + SQLx):**
- `GET /swaps?wallet=<pubkey>` and `GET /swaps?pair=<mint_a>-<mint_b>&since=<timestamp>`.
- Read-only. No write endpoints — the only writer is the ingestion pipeline.

Keep these three as separate crates/modules in a workspace (`ingestion/`, `api/`, shared `db`/`types` crate) even though it's a small project — this is the seam that would let backfill, multi-program support, etc. be added later without a rewrite (strangler-fig habit, even at this scale).

## Stack

- Rust, `tokio` async runtime.
- `axum` + `sqlx` for the API (matches existing conventions — use compile-time checked queries via `sqlx::query!` where practical).
- PostgreSQL. Local dev via Docker Compose (single `postgres` service, no separate staging config needed for a POC).
- `carbon-core` + relevant `carbon` decoder/datasource crates for ingestion.
- Yellowstone gRPC endpoint: expect an env var (`YELLOWSTONE_GRPC_URL`, `YELLOWSTONE_X_TOKEN`) — use a free-tier provider (Triton/Helius/Shyft) for dev, don't hardcode any one provider's SDK specifics beyond the gRPC client.

## What to scaffold first

1. Cargo workspace with `ingestion`, `api`, and a shared `db` crate (migrations + shared types).
2. `docker-compose.yml` with a `postgres` service and a `.env.example`.
3. One SQLx migration creating the `swaps` table as described above.
4. Ingestion crate: connect to Yellowstone, subscribe filtered to the target program, decode via Carbon, log decoded swaps to stdout (no DB write yet) — get this loop proven before wiring persistence.
5. Wire the processor to upsert into Postgres.
6. API crate: the two GET endpoints above, reading from the same Postgres.
7. README: what it does, what it explicitly doesn't do (the non-goals list above), how to run it locally.

Stop after step 7 and hand back — the person bootstrapping this wants to take over from a working scaffold, not have every feature auto-added.

## Conventions

- No speculative abstraction — this is a POC meant to be read and understood quickly by someone evaluating it as a portfolio piece.
- Prefer explicit, boring code over clever generics, especially in the processor — the whole point is auditability.
- Every non-obvious decision (e.g. why upsert-by-signature, why no backfill) gets a one-line comment or README note, not silent implementation.
