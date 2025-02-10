# Liquidity Pool Smart Contract

This document describes the Liquidity Pool Smart Contract used in our decentralized lending protocol on the MultiversX blockchain. **Important:** This contract is deployed as a child smart contract by the Controller (or Lending) SC for each market (each unique token). Only the Controller SC is allowed to call its functions, ensuring centralized validation and consistency across all markets.

---

## Table of Contents

- [Overview](#overview)
- [Architecture & Design](#architecture--design)
- [Key Features](#key-features)
- [Functionality Summary](#functionality-summary)
  - [Initialization & Upgrade](#initialization--upgrade)
  - [Interest Accrual & State Updates](#interest-accrual--state-updates)
  - [User Operations (via Controller SC)](#user-operations-via-controller-sc)
  - [Revenue Management & External Operations](#revenue-management--external-operations)
  - [Utility, Math & View Functions](#utility-math--view-functions)
  - [StorageCache Helper](#storagecache-helper)
- [Testing](#testing)
- [Contribution](#contribution)
- [License](#license)

---

## Overview

The Liquidity Pool SC is a core component of our lending protocol. It is responsible for:

- **Managing Liquidity:**  
  Handling supply, borrow, repay, and withdrawal operations.
  
- **Accruing Interest:**  
  Using an index-based model to efficiently compute interest on both borrow and supply positions without per-user state updates.
  
- **Revenue Splitting:**  
  Dividing accrued interest into a portion that benefits suppliers and a portion reserved as protocol revenue (determined by a configurable reserve factor).

- **Integration with Controller SC:**  
  All operations in this contract are called exclusively by the Controller (or Lending) SC, which passes along the `AccountPosition` (an integral part of each NFT representing a user's market position).

---

## Architecture & Design

- **Child Contract Deployment:**  
  Each asset market has its own Liquidity Pool SC deployed as a child of the Controller SC. This design isolates market-specific state and ensures that only the Controller can modify the liquidity pool’s data.

- **Interest Rate Model:**  
  - **Dynamic Rate Computation:**  
    Rates are computed using a multi-slope model. When utilization is below an optimal threshold, the interest increases gradually; beyond this threshold, a steeper slope applies.  
  - **Interest Factor & BP:**  
    The interest factor is calculated using a liniar function over elapsed time, and the global indices (borrow and supply) compound accordingly.  
    A constant base point (BP) is used as the starting index and is expressed with **21 decimals** to ensure high precision in our calculations.

- **AccountPosition Integration:**  
  The `AccountPosition` (passed from the Controller SC) encapsulates each user's market position and is associated with an NFT. This struct contains the principal amount, accumulated interest, and other liquidation parameters.

- **State Consistency:**  
  The `StorageCache` pattern is used to cache state during operations and then commit updates atomically, reducing risks from intermediate inconsistencies or rounding errors.

---

## Key Features

- **Efficient Interest Accrual:**  
  The contract maintains global borrow and supply indexes. Interest is accrued by updating these indexes instead of iterating through every position, enabling gas-efficient compounding.

- **Configurable Rate Parameters:**  
  Parameters such as maximum rate, base rate, slopes, and optimal utilization are configurable at initialization or via upgrades. These are scaled using ManagedDecimal arithmetic to reconcile differences (e.g., USDC has 6 decimals, while our BP uses 21 decimals).

- **Protocol Revenue Management:**  
  A reserve factor (e.g., 10%, expressed as a 21-decimal number) determines how much of the accrued interest is set aside as protocol revenue. This revenue can later be claimed by the protocol owner.

- **Comprehensive Operations:**  
  The SC supports supply, borrow, repay, withdraw, flash loans, and strategy creation. All operations update market state accurately and emit events for transparency.

- **Controller-Only Access:**  
  Every function is designed to be called only by the Controller SC, ensuring that market operations are centralized and validated through a single trusted entry point.

---

## Functionality Summary

### Initialization & Upgrade

- **init:**  
  - **Purpose:** Sets up the liquidity pool for a given asset by defining its token, interest rate parameters, reserve factor, and decimal precision.
  - **Highlights:**  
    - Initializes borrow and supply indexes to BP (with 21 decimals).
    - Sets protocol revenue to zero and records the current timestamp.

- **upgrade:**  
  - **Purpose:** Allows updating the market’s core parameters (interest rates, slopes, reserve factor) to adapt to changing market conditions.
  - **Highlights:**  
    - Emits an event detailing the new parameters.
    - Updates the stored pool parameters atomically.

### Interest Accrual & State Updates

- **update_indexes:**  
  - **Purpose:** Recalculates the borrow and supply indexes based on elapsed time.
  - **Highlights:**  
    - Uses a StorageCache to batch state changes.
    - Calls `update_interest_indexes` which computes the interest factor and updates indexes.
    - Emits a market state event reflecting new indexes, reserves, and revenue.

- **update_rewards_reserves:**  
  - **Purpose:** Calculates the total interest accrued on the borrowed amount and splits it into the supplier yield and protocol fee.
  - **Highlights:**  
    - Uses the reserve factor to compute the protocol fee.
    - Adds the fee to protocol revenue and returns the net interest for updating the supply index.

### User Operations (via Controller SC)

*Note: User-initiated operations (supply, borrow, repay, withdraw) are executed by the Controller SC, which passes the `AccountPosition` (the position part of each NFT) to this contract.*

- **supply:**  
  - **Purpose:** Deposits assets into the pool, updates the depositor’s position with accrued interest, and increases the pool’s reserves.
  
- **borrow:**  
  - **Purpose:** Allows borrowing of assets by updating the borrower’s position and transferring tokens from the pool reserves.
  
- **withdraw:**  
  - **Purpose:** Handles withdrawals by computing unaccrued interest, adjusting positions, and transferring tokens back to the user.
  
- **repay:**  
  - **Purpose:** Processes loan repayments, splitting payments into principal and interest, and updating the pool’s state accordingly.
  
- **flash_loan:**  
  - **Purpose:** Provides flash loan functionality with a fee calculation and external call to the borrower’s contract.
  
- **internal_create_strategy:**  
  - **Purpose:** Simulates a flash loan strategy creation, used when tokens are flash borrowed and the fee is applied immediately.

### Protocol Revenue & External Operations

- **add_external_protocol_revenue:**  
  - **Purpose:** Accepts external payments (e.g., from collateral liquidations for vault positions) to boost protocol revenue and reserves.
  
- **claim_revenue:**  
  - **Purpose:** Allows the protocol owner to claim accrued revenue that is available in the pool.
  - **Highlights:**  
    - Calculates available revenue based on current reserves and distributes it to the owner.
  
### Utility, Math & View Functions

- **Interest Rate Math Functions:**  
  - **compute_borrow_rate, compute_deposit_rate, compute_capital_utilisation, compute_interest:**  
    Compute market rates and accrued interest using a multi-slope model. Our calculations use BP (with 21 decimals) to ensure high precision.
  
- **internal_update_position_with_interest:**  
  - **Purpose:** Updates an `AccountPosition` with newly accrued interest, ensuring that positions reflect the current state of the global indexes.
  
- **View Endpoints:**  
  - **get_capital_utilisation, get_total_capital, get_debt_interest, get_deposit_rate, get_borrow_rate:**  
    These read-only functions provide external access to computed market metrics.

### StorageCache Helper

- **Purpose:**  
  `StorageCache` is a helper struct that caches state variables (supplied amount, reserves, borrowed amount, protocol revenue, indexes, timestamps) during a transaction.
  
- **Mechanism:**  
  Its Drop implementation commits all cached changes back to on-chain storage atomically, ensuring consistency and reducing risks of rounding errors.

---

## Testing

The Liquidity Pool SC is thoroughly tested as part of the Controller SC tests. These integration tests simulate complete market flows—including supply, borrow, repay, withdraw, and flash loan operations—to verify that interest accrual, revenue splitting, and state consistency are maintained under various scenarios.

---

## Contribution

Contributions to the Liquidity Pool SC are welcome. Please follow these guidelines:

1. Fork the repository and create a feature branch.
2. Write tests and update documentation for any changes.
3. Submit a pull request for review.

---

## License

This project is licensed under the MIT License.
