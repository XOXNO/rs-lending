# 🔮 Oracle Module: Secure & Efficient Price Discovery

## Overview

The **Oracle Module** is a **high-performance, gas-optimized price oracle** designed for **accurate and manipulation-resistant price discovery** in the lending protocol. It ensures that **prices are fetched, validated, and cached efficiently**, reducing gas costs while maintaining **high security standards**.

---

## 🚀 Key Features

### 🏦 Multi-Source Price Fetching
- **Primary Sources:** Prices are pulled from **on-chain aggregators, DEX pairs, and price oracle contracts**.
- **Failsafe Mechanism:** If the **primary price source is unavailable**, the module **automatically falls back to secondary sources**.

### 🔄 Recursive Price Resolution
- **Multi-Hop Search:** If a token **does not have a direct EGLD pair**, the system:
  1. **Finds an intermediate pair** (e.g., `TOKEN-X → TOKEN-Y`).
  2. **Finds the final price via EGLD** (`TOKEN-Y → EGLD`).
  3. **Derives the TOKEN-X price based on EGLD**.

### ⚡ Gas-Optimized Caching
- **Transaction-Level Cache:** Prices fetched **within the same transaction are cached**, **avoiding duplicate calls** to external contracts.
- **Efficient Price Retrieval:** If a **price is already available in cache**, it is used **instantly**, reducing gas fees.

### 🏰 Secure TWAP & Safe Pricing
- **TWAP Protection:** The Oracle Module integrates with **Time-Weighted Average Price (TWAP) feeds** from **DEXs**, ensuring **prices cannot be manipulated through flash loans**.
- **Safe Price Mode:** If **price deviations exceed the pre-set tolerance**, the system **defaults to long-term TWAP data**.

### 📊 Liquidity Pool Pricing
- **LP Token Pricing:** The module calculates the price of **LP tokens** based on their **reserve ratios and on-chain oracle data**.
- **Supports**: **XExchange, LXOXNO, and WEGLD pools**.

### 🛡️ Security & Validation
- **Price Fluctuation Tolerance:** Prevents **manipulated pricing** by enforcing **upper and lower price bounds**.
- **Failsafe Mechanisms:** If **multiple price sources are unavailable**, execution **halts** rather than using unreliable data.

---

## ⚙️ How It Works

### **1️⃣ Price Retrieval Flow**
1. **Check Cache:** If a valid price exists in **storage_cache**, return it immediately.
2. **Fetch from Oracle:** Query **on-chain price oracle contracts**.
3. **Primary Source Resolution:**
   - If the **token is paired with EGLD**, return the direct price.
   - If not, **search for an intermediary token** and compute its price.
4. **Recursive Price Resolution:** If **no direct price is found**, iteratively **resolve prices via multiple hops**.
5. **TWAP & Safe Pricing:** Validate **price deviation risks** to ensure **price stability**.
6. **Final Price Decision:** Select the most **secure and reliable price**.

### **2️⃣ Pricing Methods**
- **Direct Aggregator Pricing** → Pulls prices from **primary oracle sources**.
- **Safe Pricing via TWAP** → Ensures **manipulation-resistant prices**.
- **Hybrid Mix Pricing** → Uses **multiple pricing sources** for **higher accuracy**.

### **3️⃣ Recursive Price Resolution Example**
If **TOKEN-X** has **no EGLD pair**:
1. Check if **TOKEN-X → TOKEN-Y** exists.
2. Find **TOKEN-Y → EGLD** price.
3. Compute **TOKEN-X price relative to EGLD**.

---

## 🔗 Smart Contract Interactions
The Oracle Module interacts with:
- **Liquidity Pools** → Fetches **real-time and TWAP-based pricing**.
- **Safe Price Contracts** → Ensures **stable and historical price validation**.
- **DEX Aggregators** → Checks **multiple liquidity pools** for price discovery.

---

## 🏆 Why This Matters
- 🚀 **Gas-Efficient** → Uses **in-memory caching and recursive lookup**.
- 🔐 **Secure Against Attacks** → Implements **TWAP-based validation and fallback mechanisms**.
- ⚖️ **Highly Accurate** → Supports **multiple price sources** for best results.
- 🤝 **Seamless DeFi Integrations** → Works across **MultiversX, XExchange, and more**.

---

## 📩 Contact & Contributions
Want to integrate the Oracle Module? **Join the discussion on [GitHub Issues](https://github.com/)** or follow updates on **MultiversX DeFi ecosystem.**
