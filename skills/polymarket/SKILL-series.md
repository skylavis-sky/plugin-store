# Polymarket Series Markets — Full Guide

> This file is fetched on demand by Claude when series trading intent is detected.
> Canonical URL: https://raw.githubusercontent.com/okx/plugin-store/main/skills/polymarket/SKILL-series.md

---

## Supported Series (12 total: 4 assets × 3 intervals)

| Series ID | Interval | Schedule | Slug pattern |
|-----------|----------|----------|--------------|
| `btc-5m`, `eth-5m`, `sol-5m`, `xrp-5m` | 5 minutes | NYSE hours (9:30 AM–4:00 PM ET, Mon–Fri) | `{asset}-updown-5m-{unix_ts}` |
| `btc-15m`, `eth-15m`, `sol-15m`, `xrp-15m` | 15 minutes | NYSE hours | `{asset}-updown-15m-{unix_ts}` |
| `btc-4h`, `eth-4h`, `sol-4h`, `xrp-4h` | 4 hours | 24/7 (no hours check) | `{asset}-updown-4h-{unix_ts}` |

Bare asset aliases (`btc`, `bitcoin`, `eth`, `ethereum`, `sol`, `solana`, `xrp`) resolve to the 5-minute series.

Outcomes: **`up`** (price ended higher) and **`down`** (price ended lower) over the interval window.

---

## `get-series` Command

```
polymarket get-series --series <id>    # Show current + next slot
polymarket get-series --list           # List all 12 supported series
```

**Examples:**
```bash
polymarket get-series --series btc-5m      # BTC 5-min slot (NYSE hours)
polymarket get-series --series eth-15m     # ETH 15-min slot (NYSE hours)
polymarket get-series --series btc-4h      # BTC 4-hour slot (24/7)
polymarket get-series --list               # All 12 series
```

**Output fields:** `series`, `asset`, `interval`, `session` (in/out of trading hours + time remaining), `current_slot`, `next_slot`, `tip`

Each slot contains: `slug`, `condition_id`, `start`, `end`, `seconds_remaining`, `accepting_orders`, `outcomes` (map of outcome → `{token_id, price}`), `liquidity`, `volume_24hr`, `best_bid`, `best_ask`, `last_trade_price`

**Outside trading hours (5m/15m):** The command still returns slot info but `accepting_orders: false`. The `session` field shows time until the next session opens.

---

## Trading on a Series

### Option A — Use the series ID directly (recommended for single trades)

```bash
polymarket buy --market-id btc-5m --outcome up --amount 50
polymarket buy --market-id eth-15m --outcome down --amount 25
polymarket buy --market-id btc-4h --outcome up --amount 100
```

The binary auto-resolves the series ID to the current accepting slot at execution time. If the market is outside trading hours or transitioning between slots, the command fails with a clear message.

### Option B — Resolve first, then trade (when you want to inspect before committing)

```bash
polymarket get-series --series btc-5m
# Review current_slot: price, liquidity, seconds_remaining
polymarket buy --market-id btc-updown-5m-<unix_ts> --outcome up --amount 50
```

Use this when: you want to see current liquidity and prices before deciding, or when comparing slots.

### Option C — Fast path with `--token-id` (power users, repeated trades)

```bash
# Step 1: Get token IDs from current slot
polymarket get-series --series btc-5m
# Output: current_slot.outcomes.Up.token_id = "0xabc...", price = 0.52

# Step 2: Use --token-id to skip all market resolution
polymarket buy --token-id 0xabc... --outcome up --amount 50 --price 0.52 --dry-run

# Step 3: Confirm and submit
polymarket buy --token-id 0xabc... --outcome up --amount 50 --price 0.52
```

`--token-id` bypasses the Gamma + CLOB market lookup entirely — only the order book and fee endpoint are called (~2 HTTP calls vs 5–6 normally, saving ~500ms). **Token IDs change every slot** — re-run `get-series` at each new slot before using `--token-id`.

---

## Agent Flow (Step-by-Step)

1. **Run `get-series`** — show the user the current slot (price, liquidity, seconds remaining). Confirm they want to trade it.
2. **Preview** — run `buy --token-id <from_step_1> --outcome <up|down> --amount <usdc> --price <from_step_1> --dry-run`. Use the token ID from `current_slot.outcomes.Up.token_id` (or `Down`).
3. **Confirm** — after user approves the dry-run output, submit without `--dry-run`.
4. **Return** `order_id` and `status` from the response.

```bash
# Step 1
polymarket get-series --series btc-5m
# current_slot.outcomes.Up.token_id = "0xabc...", price = 0.52, seconds_remaining = 187

# Step 2
polymarket buy --token-id 0xabc... --outcome up --amount 50 --price 0.52 --dry-run

# Step 3 (after user confirms)
polymarket buy --token-id 0xabc... --outcome up --amount 50 --price 0.52
```

---

## Token ID Caching

**Always use `--token-id` whenever a `get-series` call has been made in the current conversation.**

After any `get-series` call, cache:
- `current_slot.outcomes.Up.token_id` and `current_slot.outcomes.Down.token_id`
- `current_slot.end_unix` (when this slot expires)

Before using cached token IDs, validate the slot is still live:
- `now < end_unix - 30` → valid, use `--token-id`
- `now ≥ end_unix - 30` → expiring; re-run `get-series` to refresh

Always pass `--price` with `--token-id` — the price from `get-series` is the current market price. Without `--price`, the binary falls back to fetching the book (negating some of the speed benefit).

**Decision rule:**

```
User wants to trade a series:
├── get-series output in this conversation AND end_unix > now + 30s?
│   └── YES → buy --token-id <cached_id> --outcome <up|down> --amount <x> --price <cached_price>
└── NO → run get-series first, then use --token-id from fresh output
```

---

## Series Intent Detection

Route to series trading when the user's message combines a **supported crypto asset** with a **short time horizon or directional framing**.

### Step 1 — Detect interval

| Interval | Trigger patterns |
|----------|-----------------|
| **5m** | `5m`, `5min`, `5-min`, `5 min`, `5 minute(s)`, `next 5 minutes`, `next few minutes`, `quick`, `intraday`, `short term`, `right now` |
| **15m** | `15m`, `15min`, `15-min`, `15 min`, `15 minute(s)`, `quarter hour` |
| **4h** | `4h`, `4hr`, `4 hour`, `4-hour`, `hourly` (when no 1h mentioned), `overnight`, `evening trade` |
| **Ambiguous** | No qualifier → check profile `preferences.interval` first; if unset, ask: *"Which interval — 5m, 15m, or 4h?"* |

### Step 2 — Detect asset

Supported: `bitcoin`/`btc`, `ethereum`/`eth`, `solana`/`sol`, `xrp`/`ripple`

If no asset in the message: check profile `preferences.asset` first; if unset, ask which one.

### Step 3 — Route

```
asset + interval detected       → get-series --series <asset>-<interval>
asset only (no interval)        → use preferences.interval or ask
interval only (no asset)        → use preferences.asset or ask
neither (vague: "quick trade")  → use preferences or ask both
```

### Phrase examples by interval

*5m:* "`<token>` 5m" / "5m `<token>`" / "trade the 5m on `<token>`" / "quick `<token>` bet" / "`<token>` updown" / "play the 5-minute candle on `<token>`"

*15m:* "`<token>` 15m" / "15m `<token>`" / "trade the 15m on `<token>`" / "give me the `<token>` 15m" / "bet on `<token>` in the next 15 minutes"

*4h:* "`<token>` 4h" / "4h `<token>`" / "trade the 4h on `<token>`" / "hourly `<token>` trade" / "overnight `<token>` bet"

### Disambiguation from regular markets

| User phrase | Route |
|-------------|-------|
| "`<token>` 5m / 15m / 4h" in any order | Series — use matching interval |
| "`<token>` updown" / "up or down `<token>`" | Series — default 5m unless interval specified |
| "Bet on ETH direction right now" | Series — default 5m |
| "Will BTC hit 100k?" | Regular market — `list-markets --keyword bitcoin` |
| "BTC prediction market" (no time, no direction) | Regular market |
| "TSLA up or down today?" / "NVDA direction?" | Daily stock market — `list-markets --keyword tsla` |
| Named event (election, sports, earnings) | Regular market |
| "quick crypto bet" (no specific asset) | Ask which asset — or check `preferences.asset` |

**Key rules:**
- `<crypto-asset> + <interval>` in any order = series (unambiguous)
- Stocks/equities (TSLA, NVDA, SPX, gold) are date-slug daily markets, **not** series
- 4h series is 24/7 — safe to route at any time; 5m/15m only during NYSE hours
- When in doubt, `get-series --series btc-5m` is cheap to run and resolves ambiguity

---

## Profile-Accelerated Routing

When the user profile (`~/.config/polymarket/profile.json`) has been loaded for the session, apply these short-circuits **before** the detection steps above:

### Trigger phrase lookup (highest priority)

Check `aliases.trigger_phrases` (case-insensitive substring match) against the user's input:

```json
"trigger_phrases": {
  "btc candle": "btc-5m",
  "eth hourly": "eth-4h",
  "my usual": "btc-5m"
}
```

If matched: route directly to the stored series ID. Skip detection steps 1–3. Confirm to user: *"Routing to btc-5m (your saved shortcut). Checking current slot…"*

### Default asset + interval (fallback)

If `preferences.asset` is set and step 2 (detect asset) would have asked the user:
→ Use `preferences.asset` silently, skip the question.

If `preferences.interval` is set and step 1 (detect interval) would have asked:
→ Use `preferences.interval` silently, skip the question.

Example: `preferences.asset = "btc"`, `preferences.interval = "5m"`, user says "quick trade":
→ Proceed directly to `get-series --series btc-5m` without any clarifying questions.

### Alias learning for series

After the user trades the same series 3 times in a session, offer:
> *"You've traded btc-5m three times today. Want me to save a shortcut phrase for it? Just say the phrase you'd like to use."*

If the user provides a phrase (e.g., "btc candle"), save it to `aliases.trigger_phrases["btc candle"] = "btc-5m"` and confirm.
