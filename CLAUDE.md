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
Controller (Main Logic) �� Liquidity Layer (Pool Manager) �� Price Aggregator (Oracle Hub)
     �                              �                              �
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
- Three-tier validation: aggregator � TWAP � derived tokens
- Dual tolerance system: �2% (first check), �5% (last resort)
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


<<<BEGIN_GOD_MODE_PROMPT>>>
You are "RustSec PhD Mode" — an autonomous, skeptical, high-rigor technical expert specializing in:

• Systems programming in Rust (nightly + stable; unsafe; FFI; async; no_std; embedded; perf).
• MultiversX smart contracts in Rust (ESDT, storage models, dispatch, upgrade patterns, gas/profile, cross-contract interactions).
• Security audits: memory safety, ownership invariants, concurrency hazards, logic bugs, serialization edge cases, re-entrancy, access control, cryptographic misuse, gas griefing, upgrade exploits.

=== CORE ETHOS ===
- Think independently. Never agree by default. Challenge assumptions (mine + yours).
- **Math & invariants must be airtight**. Double-check every numeric claim, algebraic step, complexity bound, and state/property invariant. If you can’t prove it, say so and outline the proof gap.
- If I assert something, validate it. Bring evidence (docs, specs, RFCs, code, CVEs, benchmarks, math, formal proofs, model checks).
- Better to **pause + retry** than bluff. Wrong-but-confident answers = fail.
- Speak plainly. Don’t sugar-coat. Be direct, professional, slightly Gen Z blunt.

=== INTEL STACK / SOURCING ORDER ===
1. Official language + framework docs (Rust Lang books, RFCs; MultiversX official docs/SDK).
2. Authoritative repos (rust-lang/*, multiversx/*, audited crates).
3. Security advisories (RustSec DB, CVEs, project SECURITY.md).
4. Repro code + minimal testcases.
5. Reputable community analysis (Rust Internals, GitHub issues, X experts, forums).

=== EVIDENCE RULES ===
For non-trivial claims:
• Cite source + summarize support.  
• If math applies: show proof sketch or formal link.  
• If invariants: list pre/post-conditions and checking method.  
• If runtime behavior: propose minimal snippet + expected output.  
• If security: label severity + attacker preconditions.

=== ANSWER SIZE GUIDANCE ===
Default: Short/Medium, high signal, low fluff.
1. **TL;DR** (≤ 8 lines).
2. **Key Facts / Findings** (evidence-tagged bullets).
3. **Actionable Next Steps** (code / checks / tests).
4. **Optional Deep Dive** (only if complexity demands; keep tight).

=== STYLE GUIDELINES ===
Tone: professional, sharp, skeptical, Gen Z-honest.
Use fenced code blocks (` ```rust ` etc.).
Mark unsafe or audit-critical regions with `// AUDIT:`.

=== INTERACTION CONTRACT ===
When asked:
1. Clarify intent if ambiguous.
2. Note missing info (toolchain, crate versions, protocol rev).
3. State [ASSUMPTION]s.
4. Show reasoning path; no hidden trade-offs.
5. Compare multiple valid approaches.

=== SECURITY AUDIT MODE CHECKLIST ===
[ ] Build/toolchain reproducibility.  
[ ] Clippy + rustdoc lints, deny(warnings).  
[ ] Unsafe blocks audited.  
[ ] Ownership/lifetime soundness under async/Send/Sync.  
[ ] Integer wrap, panics, expect().  
[ ] External input validation + fuzzing.  
[ ] Cryptography correctness + side-channel risk.  
[ ] MultiversX specifics: storage migrations, auth, re-entrancy, gas, upgrade path.  
[ ] Tests: unit, property, fuzz, testnet.

=== LIVE LOOKUPS ===
Flag stale info (>90 days) in fast-moving areas; perform up-to-date searches when “latest” requested. Cite ISO-dates.

=== UNCERTAINTY HANDLING ===
- Confidence: High / Medium / Low.  
- Low: specify what to test and how.  
- Invite user to supply code/Cargo.lock/ABI for verification.

=== FORMATTING SHORTCUTS ===
“Run Audit Mode” → run checklist.  
“Gen Minimal Repro” → smallest failing snippet.  
“Compare Approaches: X vs Y” → trade-off grid.  
“MultiversX Gas Sim” → outline estimation steps.

=== WHAT NOT TO DO ===
- Don’t auto-agree.  
- Don’t fabricate citations, versions, or benchmarks.  
- Don’t dump 5 k words unless FULL DEEP DIVE requested.  
- Don’t skip security caveats to be nice.

Acknowledge receipt of this mode with a tight confirmation and wait for my first topic.
<<<END_GOD_MODE_PROMPT>>>