# Polymarket Orders — Full Guide

> This file is fetched on demand by Claude before buy/sell/cancel operations.
> Canonical URL: https://raw.githubusercontent.com/okx/plugin-store/main/skills/polymarket/SKILL-orders.md

---

## `buy` — Buy Outcome Shares

```
polymarket buy --market-id <id> --outcome <outcome> --amount <usdc> [options]
```

> **Amount vs shares**: `buy` takes `--amount` in **USDC.e** (dollars you spend). `sell` takes `--shares` in **outcome tokens** (shares you hold). A user saying "sell $50" means sell enough shares to receive ~$50 USDC — check their balance via `get-positions` first and convert using the current bid price.

### Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--market-id` | Market condition_id (0x-prefixed) or slug | required |
| `--outcome` | Outcome label, case-insensitive (`yes`, `no`, `trump`, `up`, `down`, etc.) | required |
| `--amount` | USDC.e to spend, e.g. `100` = $100.00 | required |
| `--price` | Limit price in (0, 1) representing probability (e.g. `0.65`). Omit for market order (FOK). | — |
| `--order-type` | `GTC` (resting limit) or `FOK` (fill-or-kill) | `GTC` |
| `--approve` | Force USDC.e approval before placing | false |
| `--dry-run` | Simulate without submitting order or triggering any on-chain approval. Prints resolved parameters and exits. | false |
| `--round-up` | If amount is too small for divisibility constraints, snap up to the minimum valid amount. Logs rounded amount to stderr; output includes `rounded_up: true`. | false |
| `--post-only` | Maker-only: reject if order would immediately cross the spread. Requires `--order-type GTC`. Qualifies for maker rebates (up to 50% of fees returned daily). Incompatible with FOK. | false |
| `--expires` | Unix timestamp (seconds, UTC) for auto-cancel. Minimum 90 seconds from now. Sets `order_type` to `GTD` automatically. | — |
| `--token-id` | Skip market lookup — provide outcome token ID directly (from `get-series` or `get-market`). Saves 3-4 HTTP round trips. Token IDs change each slot — refresh with `get-series`. | — |
| `--confirm` | Confirm a previously gated action (reserved) | false |

**Auth required:** onchainos wallet; EIP-712 order signing via `onchainos sign-message --type eip712`

### Approval behavior

If USDC.e allowance is insufficient, the plugin submits `approve(exchange, order_amount)` for **exactly the order amount** — no unlimited allowances. This fires automatically with no additional gate. **Agent confirmation before calling `buy` is the sole safety gate.**

### Amount encoding

USDC.e is 6-decimal. Amounts use GCD-based integer arithmetic to guarantee `maker_raw / taker_raw == price` exactly — floating-point rounding breaks the ratio and causes API rejection.

### Minimum order size

Three independent minimums can reject a small order:

| Minimum | Source | Applies to |
|---------|--------|------------|
| Divisibility minimum (price-dependent) | Plugin zero-amount guard | All order types |
| Share minimum (typically 5 shares) | Plugin resting-order guard | GTC/GTD/POST_ONLY below best ask |
| CLOB execution floor (~$1) | Exchange runtime | Market (FOK) orders and marketable limits |

**Agent flow on size errors:**
1. **Divisibility error** (`"rounds to 0 shares"`): compute minimum from error, present to user.
2. **Share minimum** (`"below this market's minimum of N shares"`): ask *"Minimum is N shares (≈$X). Place that instead?"* Retry with `--round-up` on confirmation.
3. **FOK floor** (~$1): present both divisibility minimum and $1 floor together in a single message with two options: (a) $1 market order or (b) resting limit below the ask. Never autonomously choose a higher amount.

### Slippage warning

When `--price` is omitted (FOK order), the fill price may be significantly worse than mid on low-liquidity markets or large sizes. Recommend `--price` (limit order) for amounts above $10.

### Short-lived market warning

Check `end_date` in `get-market` output before placing resting (GTC) orders. A market resolving in < 24 hours may resolve before a limit order fills — use FOK or warn the user.

**Output fields:** `order_id`, `status` (live/matched/unmatched), `condition_id`, `outcome`, `token_id`, `side`, `order_type`, `limit_price`, `usdc_amount`, `shares`, `tx_hashes`

**Examples:**
```bash
polymarket buy --market-id will-btc-hit-100k-by-2025 --outcome yes --amount 50 --price 0.65
polymarket buy --market-id presidential-election-winner-2024 --outcome trump --amount 50 --price 0.52
polymarket buy --market-id 0xabc... --outcome no --amount 100
polymarket buy --market-id btc-5m --outcome up --amount 50  # series auto-resolve
polymarket buy --token-id 0xabc... --outcome up --amount 50 --price 0.52  # fast path
```

---

## `sell` — Sell Outcome Shares

```
polymarket sell --market-id <id> --outcome <outcome> --shares <n> [options]
```

### Flags

| Flag | Description | Default |
|------|-------------|---------|
| `--market-id` | Market condition_id or slug | required |
| `--outcome` | Outcome label, case-insensitive | required |
| `--shares` | Number of shares to sell, e.g. `250.5` | required |
| `--price` | Limit price in (0, 1). Omit for market order (FOK). | — |
| `--order-type` | `GTC` or `FOK` | `GTC` |
| `--approve` | Force CTF token approval before placing | false |
| `--post-only` | Maker-only: reject if order would immediately cross spread. Requires GTC. | false |
| `--expires` | Unix timestamp for auto-cancel. Minimum 90 s from now. Sets `order_type` to `GTD`. | — |
| `--dry-run` | Simulate without submitting or approving. Shows adjusted `limit_price`, `shares`, `usdc_out`. | false |
| `--confirm` | Confirm a low-price market sell previously gated | false |
| `--token-id` | Skip market lookup — same fast path as `buy --token-id`. | — |

**Auth required:** onchainos wallet; EIP-712 signing

**On-chain ops:** If CTF token allowance is insufficient, runs `setApprovalForAll(exchange, true)` — blanket ERC-1155 approval over **all** outcome tokens in the wallet (standard ERC-1155 model; per-token amounts are not supported). Always confirm the user understands this before their first sell.

> ⚠️ Market order slippage: FOK sells fill at the best available bid. On thin markets the received price may be well below mid. Recommend `--price` for sells above a few shares.

**Output fields:** `order_id`, `status`, `condition_id`, `outcome`, `token_id`, `side`, `order_type`, `limit_price`, `shares`, `usdc_out`, `tx_hashes`

**Examples:**
```bash
polymarket sell --market-id will-btc-hit-100k-by-2025 --outcome yes --shares 100 --price 0.72
polymarket sell --market-id 0xabc... --outcome no --shares 50
```

---

## Pre-sell Liquidity Check (Required Agent Step)

**Before calling `sell`, you MUST call `get-market` and assess liquidity for the outcome being sold.**

```bash
polymarket get-market --market-id <id>
```

Find the token matching the sold outcome in the `tokens[]` array. Extract `best_bid`, `best_ask`, `last_trade`, and market-level `liquidity`.

**Warn and require explicit user confirmation if ANY of the following apply:**

| Signal | Threshold | Message to user |
|--------|-----------|----------------|
| No buyers | `best_bid` null or 0 | "There are no active buyers for this outcome. Your order may not fill." |
| Price collapsed | `best_bid < 0.5 × last_trade` | "The best bid ($B) is less than 50% of last traded price ($L). You'd be selling at a significant loss." |
| Wide spread | `best_ask − best_bid > 0.15` | "The bid-ask spread is wide ($X), indicating thin liquidity. You may get a poor fill." |
| Thin market | `liquidity < 1000` | "This market has very low total liquidity ($X). Large sells will have high price impact." |

**Always show:** current `best_bid`, `last_trade`, `liquidity`, estimated USDC received (`shares × best_bid`), and a clear confirmation question.

Only call `sell` after the user explicitly confirms. **Skip this check if the user provided `--price`** — they've set their acceptable price.

---

## Safety Guards

Runtime guards built into the binary:

| Guard | Command | Trigger | Behavior |
|-------|---------|---------|----------|
| Zero-amount divisibility | `buy` | USDC amount rounds to 0 shares after GCD alignment | Exits with error + computed minimum. No approval tx. |
| Zero-amount divisibility | `sell` | Share amount rounds to 0 USDC after GCD alignment | Exits with error + computed minimum. No approval tx. |

**Agent behavior on size errors:** Never autonomously retry with a higher amount. Surface the error and minimum to the user; ask for explicit confirmation before retrying with `--round-up`. The `min_order_size` API field is unreliable — never use it to auto-escalate order size.

---

## `cancel` — Cancel Open Orders

```
polymarket cancel --order-id <id>
polymarket cancel --market <condition_id>
polymarket cancel --all
```

| Flag | Description |
|------|-------------|
| `--order-id` | Cancel a single order by its 0x-prefixed hash |
| `--market` | Cancel all orders for a specific market (by condition_id) |
| `--all` | Cancel ALL open orders — use with extreme caution |

**Auth required:** Yes — credentials auto-derived from onchainos wallet

> **Open orders only**: `cancel` only affects orders that have not yet filled, partially filled, or expired. Already-filled orders cannot be cancelled.

**Output fields:** `canceled` (list of cancelled order IDs), `not_canceled` (map of IDs to reasons)

**Examples:**
```bash
polymarket cancel --order-id 0xdeadbeef...
polymarket cancel --market 0xabc123...
polymarket cancel --all
```

---

## Order Type Selection Guide

Four effective order types — match user intent to the right one and proactively suggest upgrades:

| Order type | Flags | When to use |
|------------|-------|-------------|
| **FOK** (Fill-or-Kill) | *(omit `--price`)* | Trade immediately at best available price. Fills in full or not at all. |
| **GTC** (Good Till Cancelled) | `--price <x>` | Set a limit price and wait indefinitely for fill. |
| **POST_ONLY** (Maker-only GTC) | `--price <x> --post-only` | Guaranteed maker status. Qualifies for Polymarket maker rebates (up to 50% of fees returned daily). |
| **GTD** (Good Till Date) | `--price <x> --expires <unix_ts>` | Resting limit that auto-cancels at a specific time. |

### When to suggest POST_ONLY

When `--price` is below the best ask (buy) or above the best bid (sell) — the order will rest as a maker:

> *"Since this is a resting limit below the current ask, it will sit as a maker order. Polymarket returns up to 50% of fees to makers daily — want me to add `--post-only`?"*

Do **not** suggest for FOK orders (incompatible) or for marketable limit prices.

### When to suggest GTD

When user mentions a time constraint: "cancel if it doesn't fill by end of day", "good for the next hour", "don't leave this open overnight", "auto-cancel at [time]":

> *"I can set this to auto-cancel at [time] using `--expires $(date -d '[target]' +%s)`. Want me to add that?"*

Minimum expiry: **90 seconds** from now. Convert human inputs ("1 hour", "end of day") to Unix timestamp.

### Decision tree

```
User wants to trade:
├── Immediately (no price preference)         → FOK        (omit --price)
└── At a specific price (resting limit)
    ├── No time limit
    │   ├── Fee savings matter?               → POST_ONLY  (--price x --post-only)
    │   └── No preference                    → GTC        (--price x)
    └── With a time limit
        ├── Fee savings matter?               → GTD + POST_ONLY  (--price x --post-only --expires ts)
        └── No preference                    → GTD        (--price x --expires ts)
```

---

## Notes on Neg Risk Markets

Some markets (multi-outcome: NBA Finals, award shows, election categorical) use `neg_risk: true`. The plugin handles this automatically:

- `buy`: approves both `NEG_RISK_CTF_EXCHANGE` (`0xC5d563A36AE78145C45a50134d48A1215220f80a`) and `NEG_RISK_ADAPTER` (`0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296`) when USDC allowance is insufficient
- `sell`: approves both contracts via `setApprovalForAll` when CTF tokens are not approved
- Token IDs and prices function identically from the user's perspective
- `neg_risk` value is resolved from the CLOB market after Gamma lookup — the plugin handles edge cases where the Gamma API omits the field
