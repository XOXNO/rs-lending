#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

/// MarketParams defines the core parameters for a liquidity pool, including
/// the interest rate model settings and the asset’s decimal precision.
///
/// - `max_borrow_rate`: The maximum borrow rate.
/// - `base_borrow_rate`: The base borrow rate.
/// - `slope1`: The interest rate slope for utilization below the optimal threshold.
/// - `slope2`: The interest rate slope for utilization above the optimal threshold.
/// - `optimal_utilization`: The optimal utilization ratio at which the rate model transitions.
/// - `reserve_factor`: The fraction of accrued interest reserved as protocol revenue.
/// - `asset_decimals`: The number of asset_decimals for the underlying asset.
#[type_abi]
#[derive(TopEncode, TopDecode, Clone)]
pub struct MarketParams<M: ManagedTypeApi> {
    pub max_borrow_rate_ray: ManagedDecimal<M, NumDecimals>,
    pub base_borrow_rate_ray: ManagedDecimal<M, NumDecimals>,
    pub slope1_ray: ManagedDecimal<M, NumDecimals>,
    pub slope2_ray: ManagedDecimal<M, NumDecimals>,
    pub slope3_ray: ManagedDecimal<M, NumDecimals>,
    pub mid_utilization_ray: ManagedDecimal<M, NumDecimals>,
    pub optimal_utilization_ray: ManagedDecimal<M, NumDecimals>,
    pub reserve_factor_bps: ManagedDecimal<M, NumDecimals>,
    pub asset_id: EgldOrEsdtTokenIdentifier<M>,
    pub asset_decimals: usize,
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

#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum PositionMode {
    None,
    Normal,
    Multiply,
    Long,
    Short,
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
    pub position_type: AccountPositionType,
    pub asset_id: EgldOrEsdtTokenIdentifier<M>,
    pub scaled_amount_ray: ManagedDecimal<M, NumDecimals>,
    pub account_nonce: u64,
    pub liquidation_threshold_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_bonus_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_fees_bps: ManagedDecimal<M, NumDecimals>,
    pub loan_to_value_bps: ManagedDecimal<M, NumDecimals>,
}

impl<M: ManagedTypeApi> AccountPosition<M> {
    /// Creates a new AccountPosition with the specified parameters.
    ///
    /// # Parameters
    /// - `position_type`: The type of position (Deposit or Borrow).
    /// - `asset_id`: The asset identifier.
    /// - `principal_amount`: The principal amount.
    /// - `interest_accrued`: The interest accrued on the position.
    /// - `account_nonce`: A nonce for account tracking.
    /// - `creation_timestamp`: The creation timestamp.
    /// - `market_index`: The market index at the time of position creation.
    /// - `liquidation_threshold`: The liquidation threshold at entry.
    /// - `liquidation_bonus`: The liquidation bonus at entry.
    /// - `liquidation_fees`: The liquidation fees at entry.
    /// - `loan_to_value`: The loan-to-value ratio at entry.
    /// - `is_vault_position`: A flag indicating if the position is part of a vault.
    ///
    /// # Returns
    /// - `AccountPosition`: A new AccountPosition instance.
    pub fn new(
        position_type: AccountPositionType,
        asset_id: EgldOrEsdtTokenIdentifier<M>,
        scaled_amount_ray: ManagedDecimal<M, NumDecimals>,
        account_nonce: u64,
        liquidation_threshold_bps: ManagedDecimal<M, NumDecimals>,
        liquidation_bonus_bps: ManagedDecimal<M, NumDecimals>,
        liquidation_fees_bps: ManagedDecimal<M, NumDecimals>,
        loan_to_value_bps: ManagedDecimal<M, NumDecimals>,
    ) -> Self {
        AccountPosition {
            position_type,
            asset_id,
            scaled_amount_ray,
            account_nonce,
            liquidation_threshold_bps,
            liquidation_bonus_bps,
            liquidation_fees_bps,
            loan_to_value_bps,
        }
    }

    /// Converts BigUint amount to ManagedDecimal with specified scale.
    /// Used for position amount conversions in calculations.
    pub fn make_amount_decimal(
        &self,
        amount: &BigUint<M>,
        scale: usize,
    ) -> ManagedDecimal<M, NumDecimals> {
        ManagedDecimal::from_raw_units(amount.clone(), scale)
    }

    /// Returns zero value with same precision as position's scaled amount.
    /// Used for initialization and comparisons.
    pub fn zero_decimal(&self) -> ManagedDecimal<M, NumDecimals> {
        ManagedDecimal::from_raw_units(BigUint::zero(), self.scaled_amount_ray.scale())
    }

    /// Checks if position can be removed (has zero balance).
    /// Used for cleanup and position management.
    pub fn can_remove(&self) -> bool {
        self.scaled_amount_ray.into_raw_units().eq(&BigUint::zero())
    }
}

/// AssetConfig defines the risk and usage configuration for an asset in the market.
/// It includes risk parameters such as LTV, liquidation thresholds, and fees,
/// as well as supply/borrow caps and flags for collateral usage, isolation, and flashloan support.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct AssetConfig<M: ManagedTypeApi> {
    pub loan_to_value_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_threshold_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_bonus_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_fees_bps: ManagedDecimal<M, NumDecimals>,
    pub is_collateralizable: bool,
    pub is_borrowable: bool,
    pub e_mode_enabled: bool,
    pub is_isolated_asset: bool,
    pub isolation_debt_ceiling_usd_wad: ManagedDecimal<M, NumDecimals>,
    pub is_siloed_borrowing: bool,
    pub is_flashloanable: bool,
    pub flashloan_fee_bps: ManagedDecimal<M, NumDecimals>,
    pub isolation_borrow_enabled: bool,
    pub borrow_cap_wad: Option<BigUint<M>>,
    pub supply_cap_wad: Option<BigUint<M>>,
}

impl<M: ManagedTypeApi> AssetConfig<M> {
    /// Checks if asset can be used as collateral.
    /// Returns true if asset is collateralizable.
    pub fn can_supply(&self) -> bool {
        self.is_collateralizable
    }

    /// Checks if asset can be borrowed.
    /// Returns true if borrowing is enabled for this asset.
    pub fn can_borrow(&self) -> bool {
        self.is_borrowable
    }

    /// Checks if asset is in isolation mode.
    /// Isolated assets have special collateral restrictions.
    pub fn is_isolated(&self) -> bool {
        self.is_isolated_asset
    }

    /// Checks if asset has siloed borrowing restrictions.
    /// Siloed assets cannot be borrowed with other collateral types.
    pub fn is_siloed_borrowing(&self) -> bool {
        self.is_siloed_borrowing
    }

    /// Checks if asset is enabled for e-mode.
    /// E-mode allows higher capital efficiency for correlated assets.
    pub fn has_emode(&self) -> bool {
        self.e_mode_enabled
    }

    /// Checks if asset can be borrowed in isolation mode.
    /// Only specific stablecoins allowed for isolated borrowing.
    pub fn can_borrow_in_isolation(&self) -> bool {
        self.isolation_borrow_enabled
    }

    /// Checks if asset is enabled for flash loans.
    /// Returns flashloanable status.
    pub fn can_flashloan(&self) -> bool {
        self.is_flashloanable
    }

    /// Returns flash loan fee in basis points.
    /// Used to calculate fees for flash loan operations.
    pub fn flash_loan_fee(&self) -> ManagedDecimal<M, NumDecimals> {
        self.flashloan_fee_bps.clone()
    }
}

/// AssetExtendedConfigView provides an extended view of an asset's configuration,
/// including its token identifier, the full asset configuration, the market contract address,
/// and current prices in EGLD and USD.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct AssetExtendedConfigView<M: ManagedTypeApi> {
    pub asset_id: EgldOrEsdtTokenIdentifier<M>,
    pub market_contract_address: ManagedAddress<M>,
    pub price_in_egld_wad: ManagedDecimal<M, NumDecimals>,
    pub price_in_usd_wad: ManagedDecimal<M, NumDecimals>,
}

/// EModeCategory represents a risk category for e-mode assets, defining parameters like LTV and liquidation settings.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct EModeCategory<M: ManagedTypeApi> {
    pub category_id: u8,
    pub loan_to_value_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_threshold_bps: ManagedDecimal<M, NumDecimals>,
    pub liquidation_bonus_bps: ManagedDecimal<M, NumDecimals>,
    pub is_deprecated: bool,
}

impl<M: ManagedTypeApi> EModeCategory<M> {
    /// Checks if e-mode category is deprecated.
    /// Deprecated categories cannot be used for new positions.
    pub fn is_deprecated(&self) -> bool {
        self.is_deprecated
    }

    /// Returns the e-mode category ID.
    /// Used for category identification and mapping.
    pub fn id(&self) -> u8 {
        self.category_id
    }
}

/// EModeAssetConfig specifies whether an asset can be used as collateral and/or borrowed under e-mode.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeAssetConfig {
    pub is_collateralizable: bool,
    pub is_borrowable: bool,
}

impl EModeAssetConfig {
    /// Checks if asset can be borrowed in e-mode.
    /// Returns e-mode borrowing eligibility.
    pub fn can_borrow(&self) -> bool {
        self.is_borrowable
    }

    /// Checks if asset can be supplied as collateral in e-mode.
    /// Returns e-mode collateral eligibility.
    pub fn can_supply(&self) -> bool {
        self.is_collateralizable
    }
}

/// AccountAttributes encapsulates attributes related to an account’s NFT,
/// which represents a user's position in the protocol. These attributes include whether the position is isolated,
/// the e-mode category, and whether it is a vault.
#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq)]
pub struct AccountAttributes<M: ManagedTypeApi> {
    pub is_isolated_position: bool,
    pub e_mode_category_id: u8,
    pub mode: PositionMode,
    pub isolated_token: ManagedOption<M, EgldOrEsdtTokenIdentifier<M>>,
}

impl<M: ManagedTypeApi> AccountAttributes<M> {
    /// Checks if account has e-mode enabled.
    /// E-mode category ID > 0 indicates active e-mode.
    pub fn has_emode(&self) -> bool {
        self.e_mode_category_id > 0
    }

    /// Returns the account's e-mode category ID.
    /// 0 means no e-mode, >0 indicates specific e-mode category.
    pub fn emode_id(&self) -> u8 {
        self.e_mode_category_id
    }

    /// Checks if account is in isolation mode.
    /// Isolated positions have restricted collateral usage.
    pub fn is_isolated(&self) -> bool {
        self.is_isolated_position
    }

    /// Returns the isolated asset token identifier.
    /// SAFETY: Only call after verifying is_isolated() returns true.
    pub fn isolated_token(&self) -> EgldOrEsdtTokenIdentifier<M> {
        // SAFETY: This is safe because all call sites guard with is_isolated() checks
        unsafe { self.isolated_token.clone().into_option().unwrap_unchecked() }
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
    Onedex,
}

/// OracleProvider defines the configuration for an oracle provider that supplies price data.
/// It includes the tokens used, tolerance settings, the contract address of the oracle,
/// the pricing method, oracle type, source, and the asset_decimals used for prices.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct OracleProvider<M: ManagedTypeApi> {
    pub base_token_id: EgldOrEsdtTokenIdentifier<M>,
    pub quote_token_id: EgldOrEsdtTokenIdentifier<M>,
    pub tolerance: OraclePriceFluctuation<M>,
    pub oracle_contract_address: ManagedAddress<M>,
    pub pricing_method: PricingMethod,
    pub oracle_type: OracleType,
    pub exchange_source: ExchangeSource,
    pub asset_decimals: usize,
    pub onedex_pair_id: usize,
    pub max_price_stale_seconds: u64,
}
/// PriceFeedShort provides a compact representation of a token's price,
/// including the price value and the number of asset_decimals used.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct PriceFeedShort<M: ManagedTypeApi> {
    pub price_wad: ManagedDecimal<M, NumDecimals>,
    pub asset_decimals: usize,
}

/// OraclePriceFluctuation contains tolerance ratios that define acceptable price fluctuations
/// for an oracle provider. These ratios are used to safeguard against sudden market swings.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct OraclePriceFluctuation<M: ManagedTypeApi> {
    pub first_upper_ratio_bps: ManagedDecimal<M, NumDecimals>,
    pub first_lower_ratio_bps: ManagedDecimal<M, NumDecimals>,
    pub last_upper_ratio_bps: ManagedDecimal<M, NumDecimals>,
    pub last_lower_ratio_bps: ManagedDecimal<M, NumDecimals>,
}

/// MarketIndex represents the interest index for a market.
#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct MarketIndex<M: ManagedTypeApi> {
    pub borrow_index_ray: ManagedDecimal<M, NumDecimals>,
    pub supply_index_ray: ManagedDecimal<M, NumDecimals>,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct MarketIndexView<M: ManagedTypeApi> {
    pub asset_id: EgldOrEsdtTokenIdentifier<M>,
    pub supply_index_ray: ManagedDecimal<M, NumDecimals>,
    pub borrow_index_ray: ManagedDecimal<M, NumDecimals>,
    pub egld_price_wad: ManagedDecimal<M, NumDecimals>,
    pub usd_price_wad: ManagedDecimal<M, NumDecimals>,
    pub safe_price_egld_wad: ManagedDecimal<M, NumDecimals>,
    pub safe_price_usd_wad: ManagedDecimal<M, NumDecimals>,
    pub aggregator_price_egld_wad: ManagedDecimal<M, NumDecimals>,
    pub aggregator_price_usd_wad: ManagedDecimal<M, NumDecimals>,
    pub within_first_tolerance: bool,
    pub within_second_tolerance: bool,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct LiquidationEstimate<M: ManagedTypeApi> {
    pub seized_collaterals: ManagedVec<M, EgldOrEsdtTokenPayment<M>>,
    pub protocol_fees: ManagedVec<M, EgldOrEsdtTokenPayment<M>>,
    pub refunds: ManagedVec<M, EgldOrEsdtTokenPayment<M>>,
    pub max_egld_payment_wad: ManagedDecimal<M, NumDecimals>,
    pub bonus_rate_bps: ManagedDecimal<M, NumDecimals>,
}

/// PositionLimits defines the maximum number of positions an NFT can hold.
/// This limits complexity and optimizes gas costs during liquidations.
///
/// **Gas Optimization**: Liquidation operations iterate through all positions
/// for health factor calculations. By limiting positions per NFT, we ensure
/// liquidations remain within reasonable gas limits.
///
/// **Default Configuration**: 10 borrow + 10 supply = 20 total positions per NFT
#[type_abi]
#[derive(TopEncode, TopDecode, NestedEncode, NestedDecode, Clone)]
pub struct PositionLimits {
    pub max_borrow_positions: u8,
    pub max_supply_positions: u8,
}
