-- One table, one purpose: a decoded swap event.
--
-- `signature` is UNIQUE and acts as the idempotency key: the ingestion pipeline
-- upserts by signature, so replaying the same transaction (reconnects, at-least-
-- once delivery from the gRPC stream) never produces a duplicate row. This is the
-- only reorg handling v1 does — see README "What it does NOT do".
create table if not exists swaps (
    id          bigserial primary key,
    signature   text        not null unique, -- tx signature; idempotency key
    wallet      text        not null,        -- the swapper (user_source_owner)
    mint_in     text        not null,        -- input token mint
    mint_out    text        not null,        -- output token mint
    -- Canonical, order-independent pair key ("min(mint)-max(mint)") so a swap and
    -- its reverse land in the same bucket. Backs the (pair, block_time) index and
    -- the `?pair=` query. Directionality is preserved in mint_in / mint_out.
    pair        text        not null,
    amount_in   numeric     not null,        -- raw base-unit input amount
    amount_out  numeric     not null,        -- raw base-unit output amount
    price       double precision,            -- amount_out_ui / amount_in_ui (naive, nullable)
    slot        bigint      not null,
    block_time  timestamptz,                 -- from the block; may be absent on the stream
    created_at  timestamptz not null default now()
);

-- The only two query patterns v1 serves (see CLAUDE.md). No speculative indexes.
create index if not exists swaps_wallet_idx on swaps (wallet);
create index if not exists swaps_pair_time_idx on swaps (pair, block_time);
