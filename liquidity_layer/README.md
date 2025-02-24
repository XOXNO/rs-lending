# Liquidity Layer Smart Contract Documentation

This document provides a detailed overview of the **Liquidity Layer** smart contract, a critical component of the lending and borrowing protocol on the MultiversX L1 network. The Liquidity Layer manages asset pools, interest rates, and user positions, enabling users to supply assets, borrow against collateral, and manage repayments efficiently. It is designed to be modular, secure, and efficient, leveraging the MultiversX blockchain's capabilities for fast and low-cost transactions.

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

- Supply assets to earn interest.
- Borrow assets against their collateral.
- Withdraw supplied assets or repay borrowed amounts.
- Utilize flash loans for arbitrage or liquidation opportunities.

The contract manages the core mechanics of the lending protocol, including dynamic interest rates, index-based interest accrual, and flash loan execution. It interacts with a **Controller** smart contract to manage user positions and ensure consistent updates across contracts. The contract is optimized for gas efficiency using a caching mechanism and leverages MultiversX's architecture for scalability and low transaction costs.

---

## Architecture

The Liquidity Layer is built using a modular architecture to separate concerns and enhance maintainability:

- **Storage Module**: Manages on-chain storage of pool parameters, asset details, and state variables (e.g., total reserves, borrowed amounts).
- **InterestRates Module**: Computes dynamic borrow and deposit rates based on pool utilization.
- **UtilsModule**: Provides helper functions for interest calculations, index updates, and position synchronization.
- **LiquidityModule**: Handles core operations such as supplying assets, borrowing, withdrawing, repaying, and executing flash loans.
- **ViewModule**: Offers read-only endpoints for retrieving key metrics (e.g., pool utilization, interest rates).

The contract uses a **Cache** mechanism to snapshot the pool's state, reducing gas costs by minimizing on-chain reads and writes during operations.

---

## Key Concepts

### Cache Mechanism

The **Cache** struct is a critical optimization feature that enhances gas efficiency and ensures state consistency:

- **Purpose**: The cache snapshots the pool's state (e.g., reserves, indexes, timestamps) from on-chain storage into memory.
- **Operation**:
  - Operations modify the cache in memory instead of directly updating storage.
  - Changes are committed back to storage only when the cache is dropped, reducing the number of storage reads and writes.
- **Benefits**:
  - Lowers transaction costs by minimizing gas-intensive storage operations.
  - Ensures atomic updates to maintain state consistency.
- **Security**: The cache is carefully managed to prevent state inconsistencies, with updates committed atomically to storage.

### Interest Rate Model

The protocol uses a **piecewise linear interest rate model** to dynamically adjust borrow rates based on pool utilization (`u`), which is the ratio of borrowed to supplied assets. The model ensures that rates incentivize optimal pool utilization.

- **Parameters**:

  - `u_mid`: Midpoint utilization threshold.
  - `u_optimal`: Optimal utilization threshold.
  - `r_base`: Base borrow rate.
  - `r_slope1`, `r_slope2`, `r_slope3`: Slopes for different utilization ranges.
  - `r_max`: Maximum borrow rate.
  - `reserve_factor`: Fraction of interest retained by the protocol.

- **Borrow Rate Calculation**:

  - If `u < u_mid`
    $$
    \text{Borrow Rate} = r_{\text{base}} + \left( u \cdot \frac{r_{\text{slope1}}}{u_{\text{mid}}} \right)
    $$
  - If `mid < u < u_optimal`
    $$
    \text{Borrow Rate} = r_{\text{base}} + r_{\text{slope1}} + \left( (u - u_{\text{mid}}) \cdot \frac{r_{\text{slope2}}}{u_{\text{optimal}} - u_{\text{mid}}} \right)
    $$
  - If  `u >= u_optimal`  
    $$
    \text{Borrow Rate} = r_{\text{base}} + r_{\text{slope1}} + r_{\text{slope2}} + \left( (u - u_{\text{optimal}}) \cdot \frac{r_{\text{slope3}}}{1 - u_{\text{optimal}}} \right)
    $$
  - The rate is capped at `r_max` and converted to a per-second rate for precise interest accrual.

- **Deposit Rate Calculation**:  
  $$
  \text{Deposit Rate} = u \cdot \text{Borrow Rate} \cdot (1 - \text{Reserve Factor})
  $$
  This ensures suppliers earn a portion of the interest paid by borrowers, adjusted for protocol fees.

### Index Updates

The protocol uses **indexes** to track compounded interest over time:

- **Borrow Index**: Tracks the compounded interest borrowers owe, increasing with each update.
- **Supply Index**: Tracks rewards earned by suppliers, derived from interest paid by borrowers minus protocol fees.

- **Index Update Formula**:  
  Indexes are updated using a **Taylor series approximation** for interest accrual over time (`t`), where `r` is the per-second borrow rate:  
  $$
  \text{factor} = 1 + (r \cdot t) + \frac{(r \cdot t)^2}{2} + \frac{(r \cdot t)^3}{6}
  $$
  The new index is calculated by multiplying the previous index by this factor.

- **Security**: Indexes are initialized to `RAY` (1.0) to prevent division-by-zero errors in interest calculations.

---

## Core Functions

### Supply

- **Purpose**: Allows users to deposit assets into the pool to earn interest.
- **Process**:
  1. Validates the supplied asset against the pool's accepted asset.
  2. Updates global supply and borrow indexes.
  3. Updates the supplier's position with accrued interest.
  4. Increases pool reserves and total supplied amount.
  5. Transfers assets from the user to the contract.
  6. Emits a market state event.
- **Formula**: The supplier's position amount is increased by the supplied amount.

### Borrow

- **Purpose**: Enables users to borrow assets against their collateral.
- **Process**:
  1. Updates global supply and borrow indexes.
  2. Updates the borrower's position with accrued interest.
  3. Checks for sufficient pool reserves using `require!`.
  4. Increases the borrower's debt and total borrowed amount.
  5. Deducts the borrowed amount from reserves.
  6. Transfers the borrowed assets to the user.
  7. Emits a market state event.
- **Security**: Ensures sufficient liquidity and validates collateral requirements via the Controller.

### Withdraw

- **Purpose**: Allows suppliers to retrieve their assets, handling both normal withdrawals and liquidations.
- **Process**:
  1. Caps the withdrawal amount to the supplier's total position.
  2. Calculates principal, interest, and total withdrawal amount.
  3. Adjusts for liquidation fees if applicable.
  4. Updates global indexes and the supplier's position.
  5. Deducts the withdrawal amount from reserves and total supplied.
  6. Transfers assets to the user.
  7. Emits a market state event.
- **Security**: Caps withdrawal amounts and checks liquidity using `require!`.

### Repay

- **Purpose**: Processes repayments, reducing borrower debt and handling overpayments.
- **Process**:
  1. Validates the repayment amount and asset.
  2. Updates global supply and borrow indexes.
  3. Updates the borrower's position with accrued interest.
  4. Splits repayment into principal, interest, and overpayment using `split_repay`.
  5. Updates pool reserves and total borrowed amount.
  6. Refunds overpayments to the user.
  7. Emits a market state event.
- **Security**: Ensures asset validity and handles overpayments to prevent fund loss.

### Flash Loans

- **Purpose**: Provides temporary borrowing without collateral, requiring repayment with fees in the same transaction.
- **Process**:
  1. Validates the borrowed token and reserve availability.
  2. Deducts the loan amount from reserves.
  3. Computes the required repayment with fees.
  4. Drops the cache to prevent reentrancy, executes an external call, and validates repayment.
  5. Updates reserves and protocol revenue with the repayment and fee.
  6. Emits a market state event.
- **Security**:
  - Drops the cache before external calls to prevent reentrancy attacks.
  - Enforces repayment checks using `require!`.

---

## Interaction with the Controller

The **Controller** smart contract interacts with the Liquidity Layer to manage user positions and ensure consistent state updates:

- **Position Updates**:
  - The Controller passes user positions to functions like `sync_position_interest`, which updates positions with accrued interest based on the latest indexes.
  - Updated positions are returned to the Controller, ensuring synchronized state across contracts.

- **Operation Validation**:
  - The Controller ensures users meet collateral requirements before borrowing or withdrawing.
  - It validates operations by checking user positions and passing them to the Liquidity Layer for updates.

- **State Consistency**:
  - The cache mechanism ensures that position updates are consistent with the pool's state.
  - Changes to positions and pool state are committed atomically, maintaining integrity.

- **Key Interaction**:
  - The Controller calls Liquidity Layer functions (e.g., `supply`, `borrow`, `withdraw`, `repay`) with user positions.
  - The Liquidity Layer updates and returns the positions, ensuring synchronized state across contracts.

---

## Security Considerations

The Liquidity Layer incorporates several security measures to ensure robustness:

- **Owner-Only Access**:
  - Critical functions (e.g., `update_indexes`, `claim_revenue`) are restricted to the contract owner.

- **Reentrancy Prevention**:
  - The cache is dropped before external calls in `flash_loan` to prevent reentrancy attacks.

- **Liquidity Checks**:
  - `require!` statements ensure operations like borrowing and withdrawing do not exceed available reserves.

- **Asset Validation**:
  - Ensures only the pool's accepted asset is used in operations like supplying or repaying.

- **Index Initialization**:
  - Indexes are initialized to `RAY` (1.0) to prevent division-by-zero errors in interest calculations.

- **Cache Management**:
  - The cache is carefully managed to prevent state inconsistencies, with updates committed atomically to storage.

---

## Conclusion

The **Liquidity Layer** is a robust and efficient smart contract that forms the backbone of the lending and borrowing protocol on the MultiversX L1 network. By leveraging dynamic interest rates, precise index-based interest tracking, and a cache mechanism for gas optimization, it provides a secure and scalable solution for decentralized finance. This documentation offers a comprehensive guide to its architecture, algorithms, and operations, ensuring clarity for developers, auditors, and users.