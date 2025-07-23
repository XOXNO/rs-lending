# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a MultiversX Lending Protocol - a sophisticated DeFi application built on the MultiversX blockchain. It implements a comprehensive lending and borrowing protocol with NFT-based position management, advanced risk features, and institutional-grade security.

## Build and Development Commands

### Core Build Commands
```bash
make build                # Reproducible Docker build
cargo build              # Standard Rust build
cargo test               # Run unit tests
```

### Network Operations (via Makefile)
```bash
make devnet <command>    # Execute on devnet
make mainnet <command>   # Execute on mainnet
```

### Common Development Tasks
- Deploy contracts: `deployController`, `deployPriceAggregator`, `deployTemplateMarket`
- Market operations: `createMarket`, `upgradeMarket`, `listMarkets`
- Oracle management: `createOracle`, `addOracles`, `editOracleTolerance`
- E-Mode setup: `addEModeCategory`, `addAssetToEMode`, `listEModeCategories`
- Verification: `verifyController`, `verifyMarket`, `verifyPriceAggregator`

All commands are executed through `configs/script.sh` with network-specific configurations.

## Architecture

```
Controller (Main Logic) ’ Liquidity Layer (Pool Manager) ’ Price Aggregator (Oracle Hub)
     “                              “                              “
Position NFTs              Asset Markets                  External Oracles
```

### Key Components
- **Controller** (`/controller/`): Main protocol logic, position management, liquidations
- **Liquidity Layer** (`/liquidity_layer/`): Individual market pools with scaled tokens
- **Price Aggregator** (`/price_aggregator/`): Multi-source oracle with TWAP validation
- **Common Libraries** (`/common/`): Shared math, rates, errors, events, proxies

### Mathematical Precision
- **RAY**: 1e27 (27 decimals) - Interest rate calculations
- **WAD**: 1e18 (18 decimals) - Asset amounts
- **BPS**: 10000 (4 decimals) - Percentages

## Critical Design Patterns

### Storage Organization
```rust
#[multiversx_sc::module]
pub trait StorageModule {
    #[storage_mapper("collection_name")]
    fn collection_name(&self) -> SingleValueMapper<Type>;
}
```

### Error Handling
```rust
require!(condition, Errors::ERROR_NAME);
sc_panic!(Errors::CRITICAL_ERROR);
```

### Cache Pattern (Gas Optimization)
```rust
let mut cache = self.get_cache();
// Perform operations on cache
self.set_cache(&cache);
```

### Position Limits
- Maximum 10 borrow positions per NFT
- Maximum 10 supply positions per NFT
- Prevents liquidation gas failures

## Security Guidelines

### Input Validation
- Always validate addresses: `require!(!address.is_zero(), Errors::ZERO_ADDRESS)`
- Check amounts: `require!(amount > 0, Errors::INVALID_AMOUNT)`
- Verify health factors before operations

### Oracle Security
- Three-tier validation: aggregator ’ TWAP ’ derived tokens
- Dual tolerance system: ±2% (first check), ±5% (last resort)
- 15-minute TWAP freshness requirement
- Asymmetric operations during price anomalies

### Access Control
- Liquidity layer accepts calls only from controller
- Admin functions protected by ownership checks
- Flash loan callbacks verify initiator

## Testing Strategy

### Unit Tests
```bash
cargo test                           # All tests
cargo test --package controller     # Specific package
cargo test test_name               # Specific test
```

### Integration Testing
Use `flash_mock` contract for testing flash loan scenarios and complex interactions.

## Common Development Workflows

### Adding a New Market
1. Deploy template market contract
2. Configure market parameters in `configs/devnet_market_configs.json`
3. Run `createMarket` command via Makefile
4. Set up oracles for the asset
5. Configure E-Mode if applicable

### Implementing New Features
1. Follow existing module patterns in `/common/`
2. Use proxy patterns for cross-contract calls
3. Emit events for all state changes
4. Add comprehensive error handling
5. Include unit tests in the same module

### Debugging Oracle Issues
1. Check aggregator prices: `verifyPriceAggregator`
2. Verify TWAP freshness (must be < 15 minutes old)
3. Review tolerance violations in logs
4. Check derived token price calculations (LP tokens)

## Important Constraints

- No floating-point operations (use scaled integers)
- Gas limits require position limits (10 per type)
- Bad debt immediately socialized to suppliers
- Revenue accumulation separate from liquidity
- Flash loans must complete in single transaction