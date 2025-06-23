# MultiversX Lending Protocol

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![Build Status](https://img.shields.io/badge/build-passing-brightgreen.svg)]()
[![Coverage](https://img.shields.io/badge/coverage-90%25-brightgreen.svg)]()
[![Security Audit](https://img.shields.io/badge/security-audited-green.svg)]()

A next-generation decentralized lending and borrowing protocol built on MultiversX, featuring advanced risk management, NFT-powered positions, and sophisticated liquidation mechanisms. Designed for institutional-grade security and capital efficiency.

## Overview

The MultiversX Lending Protocol is a comprehensive DeFi solution that enables:

- **Dynamic Lending & Borrowing**: Supply assets to earn yield or borrow against collateral with algorithmically-determined interest rates
- **NFT-Powered Position Management**: Each lending position is represented as an NFT, enabling multiple isolated positions per wallet
- **Advanced Risk Management**: E-Mode, isolated markets, and siloed borrowing for optimal capital efficiency
- **Sophisticated Liquidation Engine**: Dutch auction mechanism with algebraic models targeting optimal health factors
- **Flash Loan Infrastructure**: Uncollateralized loans for arbitrage, refinancing, and complex DeFi strategies

## Architecture

### Core Components

```
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   Controller    │    │ Liquidity Layer  │    │ Price Aggregator│
│   (Main Logic)  │◄──►│  (Pool Manager)  │◄──►│  (Oracle Hub)   │
└─────────────────┘    └──────────────────┘    └─────────────────┘
         │                        │                        │
         ▼                        ▼                        ▼
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│ Position NFTs   │    │   Asset Markets  │    │ External Oracles│
│ (Account Mgmt)  │    │ (Individual Pools)│    │  (Price Feeds)  │
└─────────────────┘    └──────────────────┘    └─────────────────┘
```

### Mathematical Foundations

The protocol operates with high-precision arithmetic:

- **RAY Precision**: 27 decimals (1e27) for interest calculations
- **WAD Precision**: 18 decimals (1e18) for asset amounts
- **BPS Precision**: 4 decimals (10,000) for percentages

#### Interest Rate Model

The protocol uses a multi-slope interest rate model:

```
rate = base_rate + (utilization_rate × slope1)  [if utilization ≤ mid_utilization]
rate = base_rate + slope1 + ((utilization_rate - mid_utilization) × slope2)  [if mid_utilization < utilization ≤ optimal_utilization]
rate = base_rate + slope1 + slope2 + ((utilization_rate - optimal_utilization) × slope3)  [if utilization > optimal_utilization]
```

Where:
- `utilization_rate = total_debt / (total_supply + total_debt)`
- Interest compounds continuously using exponential calculations

#### Health Factor Calculation

```
health_factor = weighted_collateral_value / total_debt_value
```

A position becomes liquidatable when `health_factor < 1.0`.

## Security Features

### Multi-Layered Oracle Protection

- **TWAP Integration**: Time-weighted average prices to prevent manipulation
- **Deviation Tolerance**: Configurable price deviation limits (0.5% - 50% first tolerance, 1.5% - 100% last tolerance)
- **Multiple Oracle Sources**: Support for various price feed providers
- **Circuit Breakers**: Automatic pausing on extreme price movements

### Liquidation Mechanisms

#### Dutch Auction Model

The protocol implements a sophisticated liquidation system targeting health factors of 1.02/1.01:

```
liquidation_bonus = min(
    MAX_LIQUIDATION_BONUS,
    linear_scaling × (1 - health_factor) × K_SCALING_FACTOR
)
```

Where:
- `MAX_LIQUIDATION_BONUS = 15%`
- `K_SCALING_FACTOR = 200%`
- Dynamic bonus scaling prevents over-liquidation

#### Bad Debt Management

- **Threshold Detection**: Positions below $5 USD trigger bad debt cleanup
- **Immediate Socialization**: Bad debt is distributed across all suppliers through supply index reduction
- **Proportional Impact**: Bad debt impact is proportional to each supplier's share

### Risk Isolation

#### E-Mode (Efficiency Mode)

For correlated assets within the same risk category:
- Higher LTV ratios (up to 92.5%)
- Lower liquidation thresholds (as low as 95.5%)
- Reduced liquidation bonuses (1.5%)

#### Market Types

1. **Standard Markets**: Cross-collateral borrowing with standard risk parameters
2. **Isolated Markets**: Single collateral type per position
3. **Siloed Markets**: Restricted borrowing to specific asset types

#### Position Limits

**Gas Optimization for Liquidations**: The protocol implements governance-controlled position limits to ensure efficient liquidation operations:

- **Maximum Positions per NFT**: 10 borrow + 10 supply = 20 total positions
- **Liquidation Gas Protection**: Limits prevent positions from becoming unliquidatable due to gas constraints
- **Health Factor Calculations**: Each liquidation must iterate through all positions to calculate health factors
- **Protocol Safety**: Excessive positions could cause denial-of-service during liquidation attempts

**Default Limits**:
```json
{
  "max_borrow_positions": 10,
  "max_supply_positions": 10
}
```

**Governance Control**: Position limits are adjustable by protocol governance to accommodate:
- Network gas limit changes
- Optimization improvements
- Market condition requirements
- Emergency adjustments

## Installation & Deployment

### Prerequisites

- Rust 1.75+
- MultiversX SDK (`mxpy`)
- `jq` for JSON processing

### Local Development

```bash
# Clone the repository
git clone https://github.com/your-org/rs-lending.git
cd rs-lending

# Install dependencies
cargo build

# Run tests
cargo test

# Build WebAssembly contracts
make build-all
```

### Network Deployment

```bash
# Make scripts executable
chmod +x configs/script.sh

# Deploy to devnet
make devnet deployController
make devnet createMarket EGLD

# Deploy to mainnet
make mainnet deployController
make mainnet createMarket EGLD
```

## Usage

### For Users

#### Supplying Assets

```rust
// Supply EGLD to earn yield
controller.supply(token_id: "EGLD", amount: 1000000000000000000u64) // 1 EGLD
```

#### Borrowing Against Collateral

```rust
// Borrow USDC against EGLD collateral
controller.borrow(
    account_nft_nonce: 1,
    token_id: "USDC",
    amount: 500000000u64  // 500 USDC
)
```

#### Flash Loans

```rust
// Execute flash loan strategy
controller.flash_loan(
    receiver: address,
    assets: vec!["EGLD"],
    amounts: vec![1000000000000000000u64],
    params: encoded_params
)
```

### For Liquidators

#### Liquidation Execution

```rust
// Liquidate unhealthy position
controller.liquidate(
    account_nft_nonce: 123,
    debt_token_id: "USDC",
    debt_to_cover: 100000000u64,  // 100 USDC
    collateral_token_id: "EGLD"
)
```

## API Reference

### Core Functions

| Function | Description | Parameters |
|----------|-------------|------------|
| `supply` | Deposit assets to earn yield | `token_id`, `amount` |
| `withdraw` | Withdraw supplied assets | `account_nft_nonce`, `token_id`, `amount` |
| `borrow` | Borrow against collateral | `account_nft_nonce`, `token_id`, `amount` |
| `repay` | Repay borrowed assets | `account_nft_nonce`, `token_id`, `amount` |
| `liquidate` | Liquidate unhealthy positions | `account_nft_nonce`, `debt_token_id`, `debt_to_cover`, `collateral_token_id` |
| `flash_loan` | Execute uncollateralized loan | `receiver`, `assets`, `amounts`, `params` |

### Governance Functions

| Function | Description | Parameters |
|----------|-------------|------------|
| `setPositionLimits` | Configure position limits per NFT | `max_borrow_positions`, `max_supply_positions` |
| `editAssetConfig` | Update asset risk parameters | `asset`, `ltv`, `liquidation_threshold`, etc. |
| `addEModeCategory` | Create new e-mode category | `ltv`, `liquidation_threshold`, `liquidation_bonus` |
| `setAggregator` | Set price aggregator address | `aggregator_address` |

### View Functions

| Function | Description | Returns |
|----------|-------------|---------|
| `get_user_position` | Get account position details | `AccountPosition` |
| `get_market_configuration` | Get market parameters | `MarketConfiguration` |
| `calculate_health_factor` | Calculate position health | `ManagedDecimal` |
| `get_user_account_data` | Get comprehensive account data | `UserAccountData` |
| `getPositionLimits` | Get current position limits | `PositionLimits` |

## Configuration

### Market Parameters

Markets are configured with the following parameters:

```json
{
  "EGLD": {
    "ltv": "7500",                    // 75.00% loan-to-value
    "liquidation_threshold": "8000",   // 80.00% liquidation threshold
    "liquidation_bonus": "550",        // 5.50% liquidation bonus
    "borrow_cap": "2000000",          // 2M EGLD borrow cap
    "supply_cap": "2000000",          // 2M EGLD supply cap
    "base_rate": "1",                 // 1% base interest rate
    "reserve_factor": "500"           // 5.00% reserve factor
  }
}
```

### E-Mode Categories

```json
{
  "1": {
    "name": "EGLD Derivatives",
    "ltv": "9250",                    // 92.50%
    "liquidation_threshold": "9550",   // 95.50%
    "liquidation_bonus": "150",        // 1.50%
    "assets": {
      "EGLD": {
        "can_be_collateral": "0x01",
        "can_be_borrowed": "0x01"
      }
    }
  }
}
```

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test module
cargo test liquidations

# Run with coverage
cargo test --coverage
```

### Test Coverage

The protocol maintains >90% test coverage across:
- Core lending/borrowing functionality
- Liquidation mechanisms
- Oracle price feeds
- Risk management features
- Edge cases and error conditions

## Security Considerations

### Audited Components

- ✅ Interest rate calculations
- ✅ Liquidation algorithms
- ✅ Oracle price feeds
- ✅ Flash loan mechanisms
- ✅ Position management
- ✅ Mathematical precision

### Known Limitations

- Oracle dependency for price feeds
- Gas optimization ongoing
- Governance mechanisms in development

## Contributing

We welcome contributions! Please see our [Contributing Guidelines](CONTRIBUTING.md) for details.

### Development Workflow

1. Fork the repository
2. Create a feature branch
3. Implement changes with tests
4. Submit a pull request

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

**Disclaimer**: This protocol involves financial risk. Users should understand the risks before participating. Past performance does not guarantee future results.

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
