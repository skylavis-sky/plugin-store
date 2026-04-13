# Polymarket Onboarding — Full Guide

> This file is fetched on demand by Claude for new users (session_count ≤ 1) or when setup help is requested.
> Canonical URL: https://raw.githubusercontent.com/okx/plugin-store/main/skills/polymarket/SKILL-onboarding.md

---

## Proactive Onboarding

When a user signals they are **new or just installed** — e.g. "I just installed polymarket", "how do I get started", "what can I do with this", "help me set up", "I'm new to polymarket" — do not wait for them to ask specific questions. Walk them through the steps below **one at a time**, waiting for confirmation before proceeding to the next.

Do not dump all steps at once. Guide conversationally.

---

## Quickstart — Zero to First Trade

### Step 1 — Connect your wallet

Polymarket trades are signed by an onchainos agentic wallet on Polygon. Log in with your email (OTP) or OKX Web3 API key:

```bash
# Email-based login (sends OTP to your inbox)
onchainos wallet login your@email.com

# API key login (if you have an OKX Web3 API key)
onchainos wallet login
```

Verify a Polygon address is active:

```bash
onchainos wallet addresses --chain 137
```

Your wallet address is your Polymarket identity — all orders are signed from it and your positions are attached to it. No Polymarket account or web UI sign-up needed.

Also verify `sign-message` is available (required for order signing):

```bash
onchainos wallet sign-message --help
```

If this command errors, upgrade onchainos:

```bash
onchainos upgrade
```

Do not proceed to trading until `sign-message` is confirmed working. Do not suggest workarounds (MetaMask, private key export, manual curl).

### Step 2 — Verify your region is not restricted

Polymarket is unavailable in certain jurisdictions (including the United States and OFAC-sanctioned regions). Before bridging any funds:

```bash
polymarket check-access
```

- `accessible: true` — proceed
- `accessible: false` — your IP is restricted. **Do not top up USDC.e** until you have reviewed Polymarket's Terms of Use.

### Step 3 — Top up USDC.e on Polygon

Polymarket uses **USDC.e** (bridged USDC, contract `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`) on Polygon as collateral. Check balance:

```bash
onchainos wallet balance --chain 137
```

If insufficient:
- **From another chain**: bridge USDC to Polygon via the OKX Web3 bridge or Polygon Bridge
- **From a CEX**: withdraw USDC to your Polygon address via the Polygon network
- **Minimum suggested**: $5–$10 to cover a small test trade plus gas (Polygon gas < $0.01 per tx)

> **There is no "Polymarket deposit" step.** USDC.e lives in your onchainos wallet on Polygon and is spent directly when you buy — no transfer to a Polymarket account is required or possible.

### Step 4 — Find a market and place a trade

```bash
# Browse active markets
polymarket list-markets --keyword "bitcoin"

# Get details on a specific market
polymarket get-market --market-id will-btc-hit-100k-by-2025

# Buy $5 of YES shares at market price
polymarket buy --market-id will-btc-hit-100k-by-2025 --outcome yes --amount 5

# Check your positions
polymarket get-positions
```

The first `buy` or `sell` automatically derives your Polymarket trading credentials from your wallet — no manual API key setup required.

---

## Pre-flight Checks

Run these before the first trade of a session to verify the environment is healthy.

### Step 1 — Verify `polymarket` binary

```bash
polymarket --version
```

Expected: `polymarket 0.3.0`. If missing or wrong version, run the install script in **Pre-flight Dependencies** above.

### Step 2 — Install `onchainos` CLI

> `list-markets`, `get-market`, and `get-positions` do **not** require onchainos. Skip for read-only operations.

```bash
onchainos --version 2>/dev/null || echo "onchainos not installed"
```

If not installed: https://github.com/okx/onchainos for installation instructions.

Confirm `sign-message` is available:

```bash
onchainos wallet sign-message --help
```

If missing, upgrade: `onchainos upgrade`. Do not work around a missing `sign-message`.

### Step 3 — Verify wallet has a Polygon address

```bash
onchainos wallet addresses --chain 137
```

If no address: `onchainos wallet login your@email.com` (email OTP) or `onchainos wallet login` (API key).

### Step 4 — Check USDC.e balance (buy only)

```bash
onchainos wallet balance --chain 137
```

Confirm the wallet holds sufficient USDC.e for the intended buy amount.

---

## Credential Setup

**Your onchainos wallet is your Polymarket identity — no Polymarket account registration or separate API keys are required.**

On the first `buy`, `sell`, or `cancel`, the plugin:
1. Reads your Polygon wallet address from onchainos
2. Derives Polymarket CLOB API credentials by signing a one-time challenge with your onchainos key (L1 ClobAuth)
3. Caches the derived credentials at `~/.config/polymarket/creds.json` (0600 permissions)

Credentials are re-derived automatically when the active wallet changes.

**If credentials become stale** (`buy` or `sell` returns "NOT AUTHORIZED" or "UNAUTHORIZED"): the plugin automatically clears `~/.config/polymarket/creds.json` and prompts you to re-run. To manually clear:

```bash
rm ~/.config/polymarket/creds.json
```

**Credential storage note:** `~/.config/polymarket/creds.json` is 0600 (owner read/write only). A warning is printed at startup if permissions are looser — fix with `chmod 600 ~/.config/polymarket/creds.json`. The file remains in plaintext; avoid using it on shared machines.

---

## Environment Variables (Advanced Override)

These are **not required** for standard use. Only relevant for users who already have independent Polymarket CLOB API credentials.

| Variable | Description |
|----------|-------------|
| `POLYMARKET_API_KEY` | Override: Polymarket CLOB API key UUID |
| `POLYMARKET_SECRET` | Override: Base64url-encoded HMAC secret for L2 auth |
| `POLYMARKET_PASSPHRASE` | Override: CLOB API passphrase |

When set, these take precedence over `~/.config/polymarket/creds.json`. Standard users should leave these unset and let the plugin derive credentials from onchainos automatically.
