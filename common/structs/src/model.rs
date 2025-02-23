#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};

/// PoolParams defines the core parameters for a liquidity pool, including
/// the interest rate model settings and the asset’s decimal precision.
///
/// - `r_max`: The maximum borrow rate.
/// - `r_base`: The base borrow rate.
/// - `r_slope1`: The interest rate slope for utilization below the optimal threshold.
/// - `r_slope2`: The interest rate slope for utilization above the optimal threshold.
/// - `u_optimal`: The optimal utilization ratio at which the rate model transitions.
/// - `reserve_factor`: The fraction of accrued interest reserved as protocol revenue.
/// - `decimals`: The number of decimals for the underlying asset.
#[type_abi]
#[derive(TopEncode, TopDecode, Clone)]
pub struct PoolParams<M: ManagedTypeApi> {
    pub r_max: ManagedDecimal<M, NumDecimals>,
    pub r_base: ManagedDecimal<M, NumDecimals>,
    pub r_slope1: ManagedDecimal<M, NumDecimals>,
    pub r_slope2: ManagedDecimal<M, NumDecimals>,
    pub r_slope3: ManagedDecimal<M, NumDecimals>,
    pub u_mid: ManagedDecimal<M, NumDecimals>,
    pub u_optimal: ManagedDecimal<M, NumDecimals>,
    pub reserve_factor: ManagedDecimal<M, NumDecimals>,
    pub decimals: usize,
}

/// AccountPositionType represents the type of a user's position in the pool.
/// It can either be a deposit position or a borrow position.
#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum AccountPositionType {
    None,
    Deposit,
    Borrow,
}

/// AccountPosition represents a user's position in the liquidity pool.
/// It is part of each NFT managed by the protocol and includes details such as:
/// - The position type (Deposit or Borrow).
/// - The principal amount and accrued interest.
/// - A timestamp and index to track interest accrual.
/// - Additional parameters for liquidation (threshold, bonus, fees, LTV).
#[type_abi]
#[derive(ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct AccountPosition<M: ManagedTypeApi> {
    pub deposit_type: AccountPositionType,
    pub account_nonce: u64,
    pub token_id: EgldOrEsdtTokenIdentifier<M>,
    pub amount: ManagedDecimal<M, NumDecimals>,
    pub accumulated_interest: ManagedDecimal<M, NumDecimals>,
    pub timestamp: u64,
    pub index: ManagedDecimal<M, NumDecimals>,
    pub is_vault: bool,
    pub entry_liquidation_threshold: ManagedDecimal<M, NumDecimals>,
    pub entry_liquidation_bonus: ManagedDecimal<M, NumDecimals>,
    pub entry_liquidation_fees: ManagedDecimal<M, NumDecimals>,
    pub entry_ltv: ManagedDecimal<M, NumDecimals>,
}

impl<M: ManagedTypeApi> AccountPosition<M> {
    /// Creates a new AccountPosition with the specified parameters.
    ///
    /// # Parameters
    /// - `deposit_type`: The type of position (Deposit or Borrow).
    /// - `token_id`: The asset identifier.
    /// - `amount`: The principal amount.
    /// - `accumulated_interest`: The interest accrued on the position.
    /// - `account_nonce`: A nonce for account tracking.
    /// - `timestamp`: The creation timestamp.
    /// - `index`: The market index at the time of position creation.
    /// - `entry_liquidation_threshold`: The liquidation threshold at entry.
    /// - `entry_liquidation_bonus`: The liquidation bonus at entry.
    /// - `entry_liquidation_fees`: The liquidation fees at entry.
    /// - `entry_ltv`: The loan-to-value ratio at entry.
    /// - `is_vault`: A flag indicating if the position is part of a vault.
    ///
    /// # Returns
    /// - `AccountPosition`: A new AccountPosition instance.
    pub fn new(
        deposit_type: AccountPositionType,
        token_id: EgldOrEsdtTokenIdentifier<M>,
        amount: ManagedDecimal<M, NumDecimals>,
        accumulated_interest: ManagedDecimal<M, NumDecimals>,
        account_nonce: u64,
        timestamp: u64,
        index: ManagedDecimal<M, NumDecimals>,
        entry_liquidation_threshold: ManagedDecimal<M, NumDecimals>,
        entry_liquidation_bonus: ManagedDecimal<M, NumDecimals>,
        entry_liquidation_fees: ManagedDecimal<M, NumDecimals>,
        entry_ltv: ManagedDecimal<M, NumDecimals>,
        is_vault: bool,
    ) -> Self {
        AccountPosition {
            deposit_type,
            token_id,
            amount,
            accumulated_interest,
            account_nonce,
            timestamp,
            index,
            is_vault,
            entry_liquidation_threshold,
            entry_liquidation_bonus,
            entry_liquidation_fees,
            entry_ltv,
        }
    }

    /// Returns the total position amount by summing the principal and the accrued interest.
    ///
    /// # Returns
    /// - `BigUint<M>`: The total amount in the position.
    pub fn get_total_amount(&self) -> ManagedDecimal<M, NumDecimals> {
        self.amount.clone() + self.accumulated_interest.clone()
    }

    pub fn make_amount_decimal(&self, amount: BigUint<M>) -> ManagedDecimal<M, NumDecimals> {
        ManagedDecimal::from_raw_units(amount, self.amount.scale())
    }

    pub fn zero_decimal(&self) -> ManagedDecimal<M, NumDecimals> {
        ManagedDecimal::from_raw_units(BigUint::zero(), self.amount.scale())
    }

    pub fn can_remove(&self) -> bool {
        self.get_total_amount().eq(&self.zero_decimal())
    }
}

/// AssetConfig defines the risk and usage configuration for an asset in the market.
/// It includes risk parameters such as LTV, liquidation thresholds, and fees,
/// as well as supply/borrow caps and flags for collateral usage, isolation, and flashloan support.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct AssetConfig<M: ManagedTypeApi> {
    // Basic parameters
    pub ltv: ManagedDecimal<M, NumDecimals>,
    pub liquidation_threshold: ManagedDecimal<M, NumDecimals>,
    pub liquidation_base_bonus: ManagedDecimal<M, NumDecimals>,
    pub liquidation_max_fee: ManagedDecimal<M, NumDecimals>,

    // Asset usage flags
    pub can_be_collateral: bool,
    pub can_be_borrowed: bool,

    // E-mode configuration
    pub is_e_mode_enabled: bool, // True if the asset has at least one e-mode category.

    // Isolation mode settings
    pub is_isolated: bool,
    pub debt_ceiling_usd: ManagedDecimal<M, NumDecimals>, // Maximum debt ceiling in isolation mode.

    // Siloed borrowing flag
    pub is_siloed: bool,

    // Flashloan properties
    pub flashloan_enabled: bool,
    pub flash_loan_fee: ManagedDecimal<M, NumDecimals>,

    // Borrow flags in isolation mode (typically for stablecoins)
    pub can_borrow_in_isolation: bool,

    // Caps
    pub borrow_cap: Option<BigUint<M>>, // Maximum borrowable amount.
    pub supply_cap: Option<BigUint<M>>, // Maximum supplied amount.
}

impl<M: ManagedTypeApi> AssetConfig<M> {
    pub fn can_supply(&self) -> bool {
        self.can_be_collateral
    }

    pub fn can_borrow(&self) -> bool {
        self.can_be_borrowed
    }

    pub fn is_isolated(&self) -> bool {
        self.is_isolated
    }

    pub fn has_emode(&self) -> bool {
        self.is_e_mode_enabled
    }

    pub fn can_borrow_in_isolation(&self) -> bool {
        self.can_borrow_in_isolation
    }

    pub fn can_flashloan(&self) -> bool {
        self.flashloan_enabled
    }

    pub fn get_flash_loan_fee(&self) -> ManagedDecimal<M, NumDecimals> {
        self.flash_loan_fee.clone()
    }
}

/// AssetExtendedConfigView provides an extended view of an asset's configuration,
/// including its token identifier, the full asset configuration, the market contract address,
/// and current prices in EGLD and USD.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct AssetExtendedConfigView<M: ManagedTypeApi> {
    pub token: EgldOrEsdtTokenIdentifier<M>,
    pub market_address: ManagedAddress<M>,
    pub egld_price: ManagedDecimal<M, NumDecimals>,
    pub usd_price: ManagedDecimal<M, NumDecimals>,
}

/// EModeCategory represents a risk category for e-mode assets, defining parameters like LTV and liquidation settings.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct EModeCategory<M: ManagedTypeApi> {
    pub id: u8,
    pub ltv: ManagedDecimal<M, NumDecimals>,
    pub liquidation_threshold: ManagedDecimal<M, NumDecimals>,
    pub liquidation_bonus: ManagedDecimal<M, NumDecimals>,
    pub is_deprecated: bool,
}

impl<M: ManagedTypeApi> EModeCategory<M> {
    pub fn is_deprecated(&self) -> bool {
        self.is_deprecated
    }

    pub fn get_id(&self) -> u8 {
        self.id
    }
}

/// EModeAssetConfig specifies whether an asset can be used as collateral and/or borrowed under e-mode.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeAssetConfig {
    pub can_be_collateral: bool,
    pub can_be_borrowed: bool,
}

impl EModeAssetConfig {
    pub fn can_borrow(&self) -> bool {
        self.can_be_borrowed
    }

    pub fn can_supply(&self) -> bool {
        self.can_be_collateral
    }
}

/// NftAccountAttributes encapsulates attributes related to an account’s NFT,
/// which represents a user's position in the protocol. These attributes include whether the position is isolated,
/// the e-mode category, and whether it is a vault.
#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct NftAccountAttributes {
    pub is_isolated: bool,
    pub e_mode_category: u8,
    pub is_vault: bool,
}

impl NftAccountAttributes {
    pub fn is_vault(&self) -> bool {
        self.is_vault
    }

    pub fn has_emode(&self) -> bool {
        self.e_mode_category > 0
    }

    pub fn get_emode_id(&self) -> u8 {
        self.e_mode_category
    }

    pub fn is_isolated(&self) -> bool {
        self.is_isolated
    }
}
/// PricingMethod enumerates the methods used to determine token prices.
/// - `None`: No pricing method.
/// - `Safe`: A method focused on safety, possibly averaging multiple data sources.
/// - `Instant`: Real-time pricing.
/// - `Aggregator`: Prices obtained from an aggregator.
/// - `Mix`: A combination of methods (Safe,Aggregator).
#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum PricingMethod {
    None,
    Safe,
    Instant,
    Aggregator,
    Mix,
}

/// OracleType specifies the type of oracle used for price feeds.
/// - `None`: No oracle used.
/// - `Normal`: A standard oracle.
/// - `Derived`: Prices derived from other sources for LSD tokens.
/// - `Lp`: Prices from a liquidity pool.
#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum OracleType {
    None,
    Normal,
    Derived,
    Lp,
}

/// ExchangeSource enumerates potential sources for token price data.
/// Examples include decentralized exchanges or other protocols such as xEGLD/LXOXNO.
#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum ExchangeSource {
    None,
    XExchange,
    LXOXNO,
    XEGLD,
    LEGLD,
}

/// OracleProvider defines the configuration for an oracle provider that supplies price data.
/// It includes the tokens used, tolerance settings, the contract address of the oracle,
/// the pricing method, oracle type, source, and the decimals used for prices.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct OracleProvider<M: ManagedTypeApi> {
    pub first_token_id: EgldOrEsdtTokenIdentifier<M>, // Typically EGLD.
    pub second_token_id: EgldOrEsdtTokenIdentifier<M>, // Often unused.
    pub tolerance: OraclePriceFluctuation<M>,
    pub contract_address: ManagedAddress<M>,
    pub pricing_method: PricingMethod,
    pub token_type: OracleType,
    pub source: ExchangeSource,
    pub decimals: usize,
}

/// PriceFeedShort provides a compact representation of a token's price,
/// including the price value and the number of decimals used.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct PriceFeedShort<M: ManagedTypeApi> {
    pub price: ManagedDecimal<M, NumDecimals>,
    pub decimals: usize,
}

/// OraclePriceFluctuation contains tolerance ratios that define acceptable price fluctuations
/// for an oracle provider. These ratios are used to safeguard against sudden market swings.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct OraclePriceFluctuation<M: ManagedTypeApi> {
    pub first_upper_ratio: ManagedDecimal<M, NumDecimals>,
    pub first_lower_ratio: ManagedDecimal<M, NumDecimals>,
    pub last_upper_ratio: ManagedDecimal<M, NumDecimals>,
    pub last_lower_ratio: ManagedDecimal<M, NumDecimals>,
}
