# 🚀 MultiversX Lending & Borrowing Protocol

Welcome to the future of DeFi on MultiversX—where lending, borrowing, and managing your digital assets is not only secure and efficient, but also a fun and flexible experience! Our protocol combines cutting‑edge technology with innovative risk management and NFT‑powered account positions, empowering you to take control of your financial destiny with style. 💰✨

---

## Overview

Our protocol is a modular, state‑of‑the‑art solution that lets you:

- **Lend & Borrow Dynamically:** Supply assets to earn competitive yields or borrow against your collateral with dynamic, market-responsive interest rates.
- **Harness NFT-Powered Positions:** Each lending position is an NFT, giving you the freedom to manage multiple positions across different assets and risk profiles—completely isolated from one another. Think of it as having a customizable financial dashboard that's uniquely yours! 🎨🔒
- **Leverage Advanced Risk Models:** With features like E‑Mode, isolated and siloed markets, our protocol minimizes risk while maximizing capital efficiency.
- **Enjoy Flash Loans & Gas Efficiency:** Get instant liquidity for arbitrage, refinancing, or rapid deployments, all while enjoying significant gas savings thanks to our in-transaction caching mechanism.

---

## Key Features & Benefits

### Dynamic Interest Rates & High-Precision Calculations

- **Multi‑Slope Interest Rate Model:**  
  Our rates adjust in real time based on market utilization. Whether you're borrowing or supplying, you always get fair, dynamic pricing with smooth transitions—no more one-size-fits-all rates!
- **High Precision with 21‑Decimal Basis Points:**  
  Our calculations use 21 asset_decimals for basis points (BP) to ensure every fraction of interest is accurately tracked, minimizing rounding errors and maximizing fairness. 📊

### E‑Mode: Supercharged Borrowing Power

- **What is E‑Mode?**  
  E‑Mode unlocks higher loan-to-value (LTV) ratios by allowing you to use collateral from a single risk category. When all your collateral is in the same category, you get more borrowing power—think of it as a financial cheat code (the legal kind)! 😎
- **Why Use It:**
  - **Maximized Efficiency:** More collateral equals more borrowing power.
  - **Tailored Risk:** Enjoy lower liquidation thresholds and optimized risk parameters when you stay within your chosen risk category.

### Isolated & Siloed Markets: Risk Management Redefined

- **Isolated Markets:**  
  Each position can only use one type of collateral. This isolation ensures that if one asset's value drops, it affects only that position—keeping your other positions safe and sound. 🔒
- **Siloed Markets:**  
  Borrowing is confined to a single asset type per position, preventing cross-asset risk. This design helps to contain risk and simplifies your portfolio management.

### NFT-Based Account Positions: Your Financial Identity, Reinvented

- **Multiple Positions, One Wallet:**  
  With our NFT account implementation, you can have multiple positions—each for a different asset and risk level—all managed in one secure wallet. Each position is completely isolated, so you can diversify without the fear of cross-contamination. 🎟️✨
- **Trade, Transfer, or Customize:**  
  These NFT positions aren't just static records—they're dynamic, tradable, and customizable digital assets that represent your financial identity on-chain.

### Robust Liquidation & Risk Protection Mechanisms

- **Smart Liquidation Algorithms:**  
  Our liquidation logic is designed to protect borrowers. Liquidations are executed only when necessary and are done proportionally across all collateral. This prevents one asset from being unfairly liquidated and ensures that liquidators pay just enough to restore a healthy position.
- **Dynamic Liquidation Bonus & Fees:**  
  Liquidation parameters adapt based on your health factor. The worse your position, the higher the bonus and fee adjustments, incentivizing prompt action without over-penalizing you. ⚖️
- **Proportional Collateral Seizure:**  
  Collateral is seized proportionally from all supplied assets—nobody gets to pick which asset gets liquidated, ensuring balanced risk distribution across your portfolio.

### Flash Loans & Gas Efficiency: Fast & Frugal

- **Instant Flash Loans:**  
  Need liquidity in a flash? Our flash loan feature provides on-demand access to funds (with a fee) for rapid arbitrage or refinancing—no collateral required, as long as you return the funds within the same transaction. ⚡
- **Efficient Caching:**  
  Our innovative caching mechanism stores price feeds and state variables during transactions, reducing redundant external calls and saving you gas fees while maintaining real‑time data accuracy. ⛽💡

---

## Conclusion

Our MultiversX Lending & Borrowing Protocol is not just another DeFi platform—it's a revolution in how you manage digital assets. With its advanced risk models, dynamic interest rates, NFT-based positions that let you hold multiple isolated positions, and robust liquidation protections, our protocol provides the flexibility, security, and efficiency that modern finance demands.

Join us in redefining the future of decentralized finance, where every position is uniquely yours, every transaction is optimized for cost, and risk is managed intelligently across the board. 🚀🔐💎

Experience the power of a truly next‑gen DeFi platform—secure, dynamic, and built for tomorrow.

# MultiversX Lending Protocol Network Configuration

This repository contains network-aware scripts for deploying and managing the MultiversX Lending Protocol across different networks (devnet, mainnet).

## Setup

1. Make sure you have the following dependencies installed:

   - `jq` (JSON processor)
   - `mxpy` (MultiversX SDK)
   - `make` (optional, for using Makefile targets)

2. Make the scripts executable:
   ```
   chmod +x market_configs_v2.sh
   ```

## Network Configuration

The system uses two types of configuration files:

1. **networks.json** - Contains network-specific settings:

   - Network endpoints (proxy, chain ID)
   - Contract addresses
   - Account token details
   - File paths for WASM binaries
   - Ledger configuration

2. **Network-specific market configs** - Contain market parameters specific to each network:
   - Located in `configs/`
   - `devnet_market_configs.json` - Market configurations for devnet
   - `mainnet_market_configs.json` - Market configurations for mainnet

If you need to add or modify network configurations, edit the appropriate files:

- For network infrastructure: `networks.json`
- For market parameters: The corresponding network market config file

## Usage

You can interact with the system in two ways:

### 1. Using the Makefile

The Makefile provides convenient shortcuts for common operations:

```bash
# Show help
make help

# Deploy controller on devnet
make devnet-deploy-controller

# Create market on mainnet
make mainnet-create-market MARKET=EGLD

# List available markets
make devnet-list-markets

# List available networks
make list-networks
```

### 2. Using the Script Directly

You can also use the script directly, specifying the network as an environment variable:

```bash
# Set network
export NETWORK=devnet

# Deploy controller
./market_configs_v2.sh deployController

# Create market
./market_configs_v2.sh createMarket EGLD

# For one-off commands, specify the network inline
NETWORK=mainnet ./market_configs_v2.sh list
```

## Common Operations

### Deploying the Controller

```bash
make devnet-deploy-controller
# or
make mainnet-deploy-controller
```

### Creating a New Market

```bash
make devnet-create-market MARKET=EGLD
# or
make mainnet-create-market MARKET=EGLD
```

### Upgrading a Market

```bash
make devnet-upgrade-market MARKET=EGLD
# or
make mainnet-upgrade-market MARKET=EGLD
```

### Upgrading All Markets

```bash
make devnet-upgrade-all-markets
# or
make mainnet-upgrade-all-markets
```

### Creating Token Oracles

```bash
make devnet-create-oracle MARKET=EGLD
# or
make mainnet-create-oracle MARKET=EGLD
```

### Managing Price Aggregator

```bash
# Deploy the price aggregator
make devnet-deploy-price-aggregator

# Unpause the price aggregator
make mainnet-unpause-price-aggregator

# Add oracles to the price aggregator
make devnet-add-oracles-price-aggregator
```

## Managing E-Mode Categories

E-Mode (Efficiency Mode) allows users to borrow more when using correlated assets. For example, stablecoins or assets from the same ecosystem.

### Adding E-Mode Categories

```bash
# Add a stablecoin E-Mode category
make devnet-add-emode-category ID=1

# Add an EGLD ecosystem E-Mode category
make mainnet-add-emode-category ID=2
```

### Adding Assets to E-Mode Categories

```bash
# Add USDC to stablecoin E-Mode (category 1)
make devnet-add-asset-to-emode ID=1 ASSET=USDC

# Add EGLD to ecosystem E-Mode (category 2)
make mainnet-add-asset-to-emode ID=2 ASSET=EGLD
```

### Listing E-Mode Categories

```bash
# Show all E-Mode categories and their assets
make devnet-list-emode-categories
```

## Asset Configuration Management

### Editing Asset Configuration

You can update an asset's risk parameters, flags, and caps without recreating the market:

```bash
# Update EGLD configuration
make devnet-edit-asset-config MARKET=EGLD
```

### Editing Oracle Tolerance

Update price oracle tolerance values for a specific asset:

```bash
# Update EGLD price oracle tolerance
make devnet-edit-oracle-tolerance MARKET=EGLD
```

## Configuration Structure

The system uses two configuration structures:

1. **Market Configurations** - Token-level settings including interest rates, risk parameters, and oracle settings
2. **E-Mode Configurations** - Categories of correlated assets with their own risk parameters

Example E-Mode configuration (in `devnet_market_configs.json`):

```json
"emodes": {
  "1": {
    "name": "Stablecoins",
    "ltv": "9500",
    "liquidation_threshold": "9700",
    "liquidation_bonus": "150",
    "assets": {
      "USDC": {
        "can_be_collateral": "0x01",
        "can_be_borrowed": "0x01"
      },
      "USDT": {
        "can_be_collateral": "0x01",
        "can_be_borrowed": "0x01"
      }
    }
  }
}
```

## Adding a New Network

To add a new network:

1. Edit `networks.json` and add a new network entry with all required settings
2. Create a new market config file in `configs/` named `{network}_market_configs.json`
3. Update the Makefile to include targets for the new network

Example network entry for a new testnet:

```json
"testnet": {
  "proxy": "https://testnet-gateway.multiversx.com",
  "chain_id": "T",
  "addresses": {
    "controller": "erd1...",
    ...
  },
  ...
}
```
