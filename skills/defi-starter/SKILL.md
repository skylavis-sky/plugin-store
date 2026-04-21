---
name: defi-starter
description: "Quick-start bundle for new onchainos users. Installs top featured DeFi plugins and guides you to your first on-chain trade. Trigger phrases: just installed onchainos, get started with DeFi, new to onchainos, what plugins should I install, recommend me plugins, help me get started, I want to try DeFi, what can I do with onchainos, I just set up onchainos, install top plugins, what are the best plugins, 刚安装了onchainos, 怎么开始, 推荐插件, 新用户, 帮我开始, 有什么好用的插件."
version: "1.0.0"
author: okx
license: MIT
---


## Pre-flight Dependencies (auto-injected by Plugin Store CI)

> Run once per session before first use. These checks ensure required tools are installed.

### Install onchainos CLI + Skills (auto-injected)

```bash
# 1. Install onchainos CLI
onchainos --version 2>/dev/null || curl -fsSL https://raw.githubusercontent.com/okx/onchainos-skills/main/install.sh | sh

# 2. Install onchainos skills (enables AI agent to use onchainos commands)
npx skills add okx/onchainos-skills --yes --global

# 3. Install plugin-store skills (enables plugin discovery and management)
npx skills add okx/plugin-store --skill plugin-store --yes --global
```

---


# DeFi Starter Bundle

A one-command quick-start for new onchainos users. Installs the top featured DeFi plugins and walks you through your first trade.

## Proactive Onboarding

When this skill is loaded — **immediately, without waiting for the user to ask** — run the following sequence:

### Step 1 — Install featured plugins

Run these back-to-back. Skip any that are already installed (check `plugin-store installed` first if unsure).

```bash
plugin-store install hyperliquid-plugin --yes
plugin-store install polymarket-plugin --yes
```

After each install completes, immediately read the installed skill:
```
Read file: ~/.claude/skills/hyperliquid-plugin/SKILL.md
Read file: ~/.claude/skills/polymarket-plugin/SKILL.md
```

### Step 2 — Present what was installed

Tell the user (in their language) what is now available:

> **Installed and ready:**
>
> | Plugin | What it does | First command |
> |--------|--------------|---------------|
> | **Hyperliquid** | Trade perpetuals (BTC, ETH, SOL…) with leverage on Hyperliquid's L1 DEX. Deposit USDC from Arbitrum. | `hyperliquid quickstart` |
> | **Polymarket** | Trade prediction markets — buy YES/NO outcome tokens on Polygon. | `polymarket quickstart` |

### Step 3 — Route to the chosen plugin

Ask: **"Which would you like to start with — Hyperliquid or Polymarket?"**

Then follow that plugin's Quickstart flow without further prompting:
- Hyperliquid → run `hyperliquid quickstart`, follow the Hyperliquid SKILL.md proactive onboarding
- Polymarket → run `polymarket quickstart`, follow the Polymarket SKILL.md proactive onboarding

---

## Quickstart

```bash
# Install featured plugins in one step
plugin-store install hyperliquid-plugin --yes
plugin-store install polymarket-plugin --yes
```

---

## Plugin Overview

### Hyperliquid — Perpetuals DEX

High-performance on-chain perpetuals on its own L1, settling in USDC.

**Best for:** Leveraged trading on BTC, ETH, SOL and 100+ other assets with tight spreads and no gas fees on trades.

**Prerequisites:**
- USDC on Arbitrum to deposit into Hyperliquid
- Small amount of ETH on Arbitrum for bridging gas

**Key commands:**
- `hyperliquid quickstart` — check wallet state, get guided next step
- `hyperliquid deposit --amount 50 --confirm` — bridge USDC from Arbitrum
- `hyperliquid order --coin BTC --side buy --size 0.001 --leverage 5 --confirm` — place a leveraged order
- `hyperliquid positions` — view open positions and P&L

### Polymarket — Prediction Markets

Trade YES/NO outcome tokens on real-world events (elections, sports, crypto prices) on Polygon.

**Best for:** Taking positions on events, earning on prediction accuracy, trading market sentiment.

**Prerequisites:**
- USDC on Ethereum or Polygon
- Small amount of ETH/MATIC for gas

**Key commands:**
- `polymarket quickstart` — check wallet and get guided next step
- `polymarket list-markets --limit 10` — browse open markets
- `polymarket buy --market <id> --outcome YES --amount 10 --confirm` — buy outcome tokens
- `polymarket positions` — view current holdings

---

## Keeping the Bundle Current

The set of featured plugins is maintained in the plugin-store registry. To see the current featured list:

```bash
plugin-store search featured
```

To update all installed plugins to the latest versions:

```bash
plugin-store update --all
```

---

<rules>
<must>
  - On load, immediately install featured plugins without waiting for the user to ask
  - After each plugin install, read its SKILL.md before presenting it to the user
  - Ask which plugin the user wants to start with, then follow that plugin's own onboarding flow
  - Respond in the user's language (English or Chinese)
</must>
<should>
  - If a featured plugin is already installed, skip its install step and note it's already ready
  - After routing to a plugin, hand off fully to that plugin's SKILL.md instructions — do not duplicate its onboarding here
</should>
<never>
  - Never leave the user at a blank prompt after install — always present next steps
  - Never attempt to manually construct transactions or sign messages — always use the plugin's own commands
</never>
</rules>
