# Liquidity Layer Smart Contract Documentation

This document provides a detailed overview of the **Liquidity Layer** smart contract—a core component of the lending and borrowing protocol on the MultiversX L1 network. The Liquidity Layer manages asset pools, interest rates, and user positions, enabling users to supply assets, borrow against collateral, and efficiently manage repayments. It is designed to be modular, secure, and efficient, leveraging MultiversX’s capabilities for fast, low-cost transactions.

---

## Table of Contents

1. [Introduction](#introduction)
2. [Architecture](#architecture)
3. [Key Concepts](#key-concepts)
   - [Cache Mechanism](#cache-mechanism)
   - [Interest Rate Model](#interest-rate-model)
   - [Index Updates](#index-updates)
4. [Core Functions](#core-functions)
   - [Supply](#supply)
   - [Borrow](#borrow)
   - [Withdraw](#withdraw)
   - [Repay](#repay)
   - [Flash Loans](#flash-loans)
5. [Interaction with the Controller](#interaction-with-the-controller)
6. [Security Considerations](#security-considerations)
7. [Conclusion](#conclusion)

---

## Introduction

The **Liquidity Layer** is a decentralized lending pool smart contract that enables users to:

- **Supply assets** to earn interest.
- **Borrow assets** against their collateral.
- **Withdraw supplied assets** or repay borrowed amounts.
- **Utilize flash loans** for arbitrage or liquidation opportunities.

The contract manages the core mechanics of the lending protocol, including dynamic interest rates, index-based interest accrual, and flash loan execution. It interacts with a **Controller** smart contract to manage user positions and ensure consistent state updates. Gas efficiency is optimized via a caching mechanism that minimizes on-chain reads and writes.

---

## Architecture

The Liquidity Layer is built with a modular architecture that separates concerns and enhances maintainability:

- **Storage Module**: Manages on-chain storage for pool parameters, asset details, and state variables (e.g., total reserves, borrowed amounts).
- **InterestRates Module**: Computes dynamic borrow and deposit rates based on pool utilization.
- **UtilsModule**: Provides helper functions for interest calculations, index updates, and position synchronization.
- **LiquidityModule**: Handles core operations such as supplying assets, borrowing, withdrawing, repaying, and executing flash loans.
- **ViewModule**: Offers read-only endpoints for retrieving key market metrics (e.g., pool utilization, interest rates).

A **Cache** mechanism is used to snapshot the pool's state in memory, thereby reducing gas costs by minimizing storage operations.

---

## Key Concepts

### Cache Mechanism

The **Cache** struct is an optimization feature that snapshots the pool’s state (e.g., reserves, indexes, timestamps) into memory.

- **Purpose**: Reduces gas costs by avoiding repetitive on-chain reads/writes during operations.
- **Operation**:
  - State is read once into the cache.
  - All updates are made in memory.
  - Changes are committed atomically back to storage when the cache is dropped.
- **Benefits**:
  - Lower transaction costs.
  - Atomic updates help maintain state consistency.
- **Security**: Proper cache management prevents state inconsistencies and potential reentrancy issues.

### Interest Rate Model

The protocol employs a **piecewise linear interest rate model** that dynamically adjusts borrow rates based on pool utilization (`u`, the ratio of borrowed to supplied assets). This model incentivizes optimal utilization.

- **Parameters**:
  - `base_borrow_rate`: The base interest rate.
  - `slope1`, `slope2`, `slope3`: Interest rate slopes for different utilization ranges.
  - `mid_utilization`: The utilization threshold where the first rate slope applies.
  - `optimal_utilization`: The target utilization threshold (must be less than 1.0).
  - `max_borrow_rate`: The upper limit on the borrow rate.
  - `reserve_factor`: The fraction of interest retained as protocol revenue.
- **Borrow Rate Calculation**:
  - For `u < mid_utilization`:
    $$
    \text{Borrow Rate} = r_{\text{base}} + \left( u \cdot \frac{r_{\text{slope1}}}{u_{\text{mid}}} \right)
    $$
  - For `mid_utilization ≤ u < optimal_utilization`:
    $$
    \text{Borrow Rate} = r_{\text{base}} + r_{\text{slope1}} + \left( (u - u_{\text{mid}}) \cdot \frac{r_{\text{slope2}}}{u_{\text{optimal}} - u_{\text{mid}}} \right)
    $$
  - For `u ≥ optimal_utilization`:
    $$
    \text{Borrow Rate} = r_{\text{base}} + r_{\text{slope1}} + r_{\text{slope2}} + \left( (u - u_{\text{optimal}}) \cdot \frac{r_{\text{slope3}}}{1 - u_{\text{optimal}}} \right)
    $$
  - The rate is capped at `max_borrow_rate` and converted to a per-second rate.
- **Deposit Rate Calculation**:
  $$
  \text{Deposit Rate} = u \cdot \text{Borrow Rate} \cdot (1 - \text{Reserve Factor})
  $$

### Index Updates

Indexes are used to track compounded interest:

- **Borrow Index**: Reflects the compounded interest owed by borrowers.
- **Supply Index**: Reflects rewards accrued by suppliers.
- **Update Formula**:  
  Indexes are updated using a Taylor series approximation for interest accrual over a small time interval `t`:
  $$
  \text{factor} = 1 + (r \cdot t) + \frac{(r \cdot t)^2}{2} + \frac{(r \cdot t)^3}{6}
  $$
  The new index is the product of the old index and this factor.
- **Initialization**: Both indexes are initialized to `RAY` (representing 1.0) to avoid division-by-zero errors.

---

## Core Functions

### Supply

- **Purpose**: Deposit assets into the pool to earn interest.
- **Process**:
  1. Validate the supplied asset.
  2. Update global indexes.
  3. Update the supplier’s position with accrued interest.
  4. Increase pool reserves and total supplied.
  5. Emit a market state event.

### Borrow

- **Purpose**: Borrow assets against collateral.
- **Process**:
  1. Update global indexes.
  2. Update the borrower’s position with accrued interest.
  3. Check pool liquidity.
  4. Increase borrower’s debt and total borrowed.
  5. Deduct the borrowed amount from reserves.
  6. Transfer assets to the borrower.
  7. Emit a market state event.

### Withdraw

- **Purpose**: Withdraw supplied assets or execute liquidations.
- **Process**:
  1. Cap withdrawal amount to the user’s available balance.
  2. Calculate principal, interest, and total withdrawal (adjusting for fees if necessary).
  3. Update global indexes and the user’s position.
  4. Deduct the withdrawal amount from reserves and total supplied.
  5. Transfer assets to the user.
  6. Emit a market state event.

### Repay

- **Purpose**: Repay borrow positions and handle overpayments.
- **Process**:
  1. Validate repayment amount and asset.
  2. Update global indexes.
  3. Update the borrower’s position with accrued interest.
  4. Split repayment into principal, interest, and overpayment.
  5. Update pool reserves and total borrowed.
  6. Refund any overpayment.
  7. Emit a market state event.

### Flash Loans

- **Purpose**: Provide short-term, collateral-free borrowing with mandatory same-transaction repayment.
- **Process**:
  1. Validate the requested asset and ensure sufficient reserves.
  2. Deduct the loan amount from reserves.
  3. Calculate the required repayment (principal plus fee).
  4. Drop the cache before the external call to prevent reentrancy.
  5. Execute the external call.
  6. Validate that repayment meets requirements.
  7. Update reserves and protocol revenue.
  8. Emit a market state event.
- **Security**:
  - The cache is dropped to avoid reentrancy.
  - Repayment is strictly enforced to include fees.

---

## Interaction with the Controller

The **Controller** smart contract manages user positions and interacts with the Liquidity Layer for:

- **Position Updates**:  
  It passes user positions to functions (e.g., `sync_position_interest`) that update positions with the latest interest accrual.
- **Operation Validation**:  
  It checks collateral and other requirements before calling functions like `borrow` or `withdraw`.
- **State Synchronization**:  
  The cache mechanism and atomic updates ensure that both the Controller and Liquidity Layer have a consistent view of the pool’s state.

---

## Security Considerations

Key security features include:

- **Owner-Only Access**:  
  Critical functions are restricted to the contract owner.
- **Reentrancy Prevention**:  
  The flash loan function drops its cache before external calls to mitigate reentrancy risks.
- **Liquidity and Asset Checks**:  
  Extensive use of `require!` ensures operations do not exceed available reserves and that only the correct asset is used.
- **Index Initialization**:  
  Indexes start at `RAY` (1.0) to prevent division-by-zero.
- **Cache Management**:  
  Updates are made in memory and committed atomically to storage.

---

## Conclusion

The **Liquidity Layer** smart contract forms the backbone of the lending and borrowing protocol on MultiversX. With its dynamic interest rate model, precise index-based interest accrual, and efficient cache mechanism, it offers a secure, scalable, and cost-effective solution for decentralized finance. This documentation provides a comprehensive guide to its architecture, functionality, and security features, ensuring clarity for developers, auditors, and users.
