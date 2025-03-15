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
- **High Precision with 27‑Decimal Basis Points:**  
  Our calculations use 27 asset_decimals for basis points (BP) to ensure every fraction of interest is accurately tracked, minimizing rounding errors and maximizing fairness. 📊

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
   ```bash
   chmod +x configs/script.sh
   ```

## Network Configuration

The system uses three types of configuration files:

1. **networks.json** - Contains network-specific settings:
   - Network endpoints (proxy, chain ID)
   - Contract addresses
   - Oracle addresses
   - Account token details
   - File paths for WASM binaries
   - Ledger configuration

2. **Network-specific market configs** - Contain market parameters:
   - Located in `configs/`
   - `devnet_market_configs.json` - Market configurations for devnet
   - `mainnet_market_configs.json` - Market configurations for mainnet

3. **E-Mode configuration** - Contains E-Mode categories and their settings:
   - Located in `configs/emodes.json`
   - Defines E-Mode categories per network
   - Specifies assets and their parameters for each category

## Usage


```bash
make <network> <command> [arguments]

# Examples:
make devnet createMarket EGLD
make devnet addEModeCategory 1
make devnet addAssetToEMode 1 EGLD
make devnet show EGLD
make devnet listMarkets
```

## Common Operations

### Controller Management
```bash
# Deploy controller
make devnet deployController

# Upgrade controller
make devnet upgradeController

# Register account token
make devnet registerAccountToken
```

### Market Management
```bash
# Create market
make devnet createMarket EGLD

# Upgrade specific market
make devnet upgradeMarket EGLD

# Upgrade all markets
make devnet upgradeAllMarkets

# Show market configuration
make devnet show EGLD

# List all markets
make devnet listMarkets

# Edit asset configuration
make devnet editAssetConfig EGLD
```

### Oracle Management
```bash
# Create oracle for a market
make devnet createOracle EGLD

# Edit oracle tolerance
make devnet editOracleTolerance EGLD

# Deploy price aggregator
make devnet deployPriceAggregator

# Add oracles to price aggregator
make devnet addOracles <address1> [address2] [address3] ...

# Pause/unpause price aggregator
make devnet pauseAggregator
make devnet unpauseAggregator
```

### E-Mode Management
```bash
# List E-Mode categories
make devnet listEModeCategories

# Add new E-Mode category
make devnet addEModeCategory 1

# Add asset to E-Mode category
make devnet addAssetToEMode 1 EGLD
```

### Revenue Management
```bash
# Claim revenue from all markets
make devnet claimRevenue
```

## Market Configuration Structure

Market configurations use human-readable values that are automatically scaled when used in transactions:

```json
{
  "EGLD": {
    "token_id": "EGLD",
    "ltv": "7500",                    // 75.00%
    "liquidation_threshold": "8000",   // 80.00%
    "liquidation_bonus": "550",        // 5.50%
    "borrow_cap": "20000",            // 20,000 EGLD
    "supply_cap": "20000",            // 20,000 EGLD
    "base_rate": "1",                 // 1%
    "max_rate": "69",                 // 69%
    "slope1": "5",                    // 5%
    "slope2": "15",                   // 15%
    "slope3": "50",                   // 50%
    "mid_utilization": "65",          // 65%
    "optimal_utilization": "90",      // 90%
    "reserve_factor": "2500",         // 25.00%
    "oracle_decimals": "18"           // Used for scaling caps
  }
}
```

## E-Mode Configuration Structure

```json
{
  "devnet": {
    "1": {
      "name": "EGLD Derivatives",
      "ltv": "9250",                  // 92.50%
      "liquidation_threshold": "9550", // 95.50%
      "liquidation_bonus": "150",      // 1.50%
      "assets": {
        "EGLD": {
          "can_be_collateral": "0x01",
          "can_be_borrowed": "0x01"
        }
      }
    }
  }
}
```
