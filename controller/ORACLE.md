# 🔮 Oracle Module: Secure & Efficient Price Discovery

## Overview

The **Oracle Module** is a vital component of the lending protocol, engineered to deliver **accurate, manipulation-resistant price data** for assets. It ensures dependable and secure price feeds, safeguarding against market volatility and manipulation attempts. The system is **gas-efficient** and **highly configurable**, accommodating multiple price sources and validation mechanisms, including **Time-Weighted Average Price (TWAP)** and **off-chain price aggregator feeds**. Prices are rigorously validated against **tolerance ranges** to guarantee stability and security.

---

## 🚀 Key Features

### 🏦 Multi-Source Price Fetching

- **Primary Sources:** Prices are retrieved from:
  - **On-chain aggregators** (e.g., off-chain prices converted to on-chain data).
  - **DEX pairs** (e.g., XExchange, LXOXNO).
  - **Price oracle contracts**.
- **Fallback Mechanism:** If a primary source is unavailable or fails validation, the system switches to **secondary sources** or **TWAP-based pricing**.

### 🔄 Recursive Price Resolution

- **Multi-Hop Search:** For tokens lacking a direct EGLD pair:
  1. Identifies an **intermediate token** with a known pair.
  2. Calculates the price via **multiple hops** (e.g., `TOKEN-X → TOKEN-Y → EGLD`).
- **Efficient Resolution:** Ensures price discovery for tokens with indirect liquidity paths.

### ⚡ Gas-Optimized Caching

- **Transaction-Level Cache:** Prices fetched within the same transaction are stored in **cache**, eliminating redundant external calls.
- **Efficient Retrieval:** Cached prices are reused instantly, reducing gas costs and enhancing performance.

### 🏰 Secure TWAP & Safe Pricing

- **TWAP Protection:** Utilizes **Time-Weighted Average Price (TWAP)** from DEXs to prevent flash loan-based price manipulation.
- **Safe Price Mechanism (XExchange):** Employs XExchange’s **Safe Price** feature, a TWAP-based pricing method, to provide secure and stable price data.
- **Configurable Tolerances:** Allows setting **upper and lower bounds** for price fluctuations to ensure stability.

### 📊 Liquidity Pool (LP) and LSD Token Pricing

- **LP Token Pricing:** Calculates prices for LP tokens based on **reserve ratios** and underlying asset prices.
  - Supports **XExchange, LXOXNO, and other DEX pools**.
  - Validates LP token prices against **TWAP data** for manipulation resistance.
- **LSD Token Pricing:** Computes prices for Liquid Staking Derivative (LSD) tokens (e.g., xEGLD, LXOXNO) using their specific exchange rates or derived pricing mechanisms.
  - **xEGLD and LXOXNO** are distinct from LP tokens, treated as derivatives of staked assets.

### 🛡️ Security & Validation

- **Price Fluctuation Tolerance:** Prevents manipulation by enforcing **pre-configured tolerance ranges** for price deviations.
  - Short-term and long-term prices must remain within **upper and lower tolerance ratios**.
  - If prices deviate excessively, the system reverts to **safe prices** or halts operations if unsafe pricing is disallowed.
- **Failsafe Mechanisms:** Halts operations if multiple sources fail or prices deviate excessively, avoiding reliance on unreliable data.
- **Anchor Checks:** Validates real-time prices (e.g., aggregator prices) against **safe price anchors** (e.g., TWAP data) for consistency.

---

## ⚙️ How Prices Are Computed and Protected

### **1️⃣ Price Retrieval Flow**

1. **Cache Check:**
   - Returns a valid price from the **transaction-level cache** if available.
2. **Oracle Query:**
   - Fetches price data from the configured **on-chain price oracle**, **aggregator**, or **DEX pair**.
3. **Primary Source Resolution:**
   - Computes prices directly for tokens with **direct EGLD pairs**.
   - Uses **recursive resolution** for tokens without direct pairs (e.g., `TOKEN-X → TOKEN-Y → EGLD`).
4. **TWAP & Safe Pricing Validation:**
   - Compares real-time prices with **TWAP data** to detect anomalies.
   - Falls back to **safe prices** (e.g., XExchange’s Safe Price) if deviations exceed tolerances.
5. **Final Price Selection:**
   - Selects the most **secure and reliable price** based on validation checks.

---

### **2️⃣ Pricing Methods**

The Oracle Module supports multiple pricing methods, each with tailored validation and protection:

#### **Aggregator Pricing (Off-Chain Pushed Prices)**

- **Description:** Retrieves real-time prices from **on-chain aggregators**.
- **Validation:**
  - Compares prices against **TWAP-based safe prices**.
  - Ensures prices stay within **tolerance ranges** relative to TWAP data.
- **Protection:**
  - Falls back to **safe prices** (e.g., TWAP) if aggregator prices deviate beyond tolerances.

#### **Safe Pricing (TWAP)**

- **Description:** Computes **Time-Weighted Average Prices** over configurable intervals (e.g., 10 minutes, 1 hour) via XExchange’s **Safe Price** mechanism.
- **Validation:**
  - Compares short-term TWAP (e.g., 10 minutes) against long-term TWAP (e.g., 1 hour).
  - Ensures prices stay within **pre-configured tolerance ranges**.
- **Protection:**
  - Uses **long-term TWAP** or halts operations if deviations are excessive and unsafe pricing is disallowed.

#### **Hybrid Pricing (Mix of Aggregator and TWAP)**

- **Description:** Combines **aggregator prices** and **TWAP data** for enhanced accuracy and security.
- **Validation:**
  - Validates aggregator prices against TWAP-based safe prices within **tolerance ranges**.
- **Protection:**
  - Falls back to **safe prices** or halts if deviations exceed tolerances and unsafe pricing is disallowed.

---

### **3️⃣ LP Token Pricing**

- **Computation:**
  - Calculates LP token prices using **reserve ratios** and underlying asset prices from DEX pools.
- **Validation:**
  - Compares short-term LP prices against long-term TWAP data within **tolerance ranges**.
- **Protection:**
  - Falls back to **long-term TWAP LP prices** or halts if deviations are excessive.

---

### **4️⃣ LSD Token Pricing (e.g., xEGLD, LXOXNO)**

- **Derived Pricing:**
  - Computes prices based on **exchange rates** from staking contracts (e.g., xEGLD, LXOXNO).
  - Derives LSD token prices using the underlying asset price (e.g., EGLD) and staking contract data.
- **Validation:**
  - Validates prices against **TWAP data** or **aggregator prices** for consistency.

---

## 🔗 Smart Contract Interactions

- **Price Aggregators:** Fetches and validates real-time prices against TWAP data.
- **DEX Pairs:** Queries **XExchange, LXOXNO** for liquidity-based pricing with TWAP integration.
- **Safe Price Contracts (XExchange):** Uses XExchange’s **Safe Price** for TWAP-based pricing.
- **Staking Contracts:** Retrieves **exchange rates** for LSD token pricing (e.g., xEGLD, LXOXNO).

---

## 🏆 Why This Matters

- 🚀 **Gas-Efficient:** Reduces costs with caching and recursive lookups.
- 🔐 **Manipulation-Resistant:** Protects against attacks with TWAP and tolerance checks.
- ⚖️ **Configurable:** Adapts to various pricing methods and tolerance levels.
- 🤝 **Ecosystem Integration:** Seamlessly supports the **MultiversX DeFi ecosystem**.

---

## 📩 Contact & Contributions

- **GitHub Issues:** Discuss on [GitHub Issues](https://github.com/).
- **MultiversX DeFi Updates:** Stay informed about ecosystem developments.

---
