#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub const SECONDS_PER_YEAR: u64 = 31_556_926;
pub const BP: u128 = 1_000_000_000_000_000_000_000; // Represents 100%
pub const DECIMAL_PRECISION: usize = 21;
pub const MAX_BONUS: u128 = 300_000_000_000_000_000_000; // Represents 30% basis points

#[type_abi]
#[derive(TopEncode, TopDecode)]
pub struct PoolParams<M: ManagedTypeApi> {
    pub r_max: ManagedDecimal<M, NumDecimals>,
    pub r_base: ManagedDecimal<M, NumDecimals>,
    pub r_slope1: ManagedDecimal<M, NumDecimals>,
    pub r_slope2: ManagedDecimal<M, NumDecimals>,
    pub u_optimal: ManagedDecimal<M, NumDecimals>,
    pub reserve_factor: ManagedDecimal<M, NumDecimals>,
    pub decimals: usize,
}

#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub enum UpdateAssetParamsType {
    None,
    LTV,
    LiquidationBonus,
    LiquidationThreshold,
}

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
#[derive(ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct AccountPosition<M: ManagedTypeApi> {
    pub deposit_type: AccountPositionType,
    pub account_nonce: u64,
    pub token_id: EgldOrEsdtTokenIdentifier<M>,
    pub amount: BigUint<M>,
    pub accumulated_interest: BigUint<M>,
    pub timestamp: u64,
    pub index: BigUint<M>,
    pub is_vault: bool,

    pub entry_liquidation_threshold: BigUint<M>,
}

impl<M: ManagedTypeApi> AccountPosition<M> {
    pub fn new(
        deposit_type: AccountPositionType,
        token_id: EgldOrEsdtTokenIdentifier<M>,
        amount: BigUint<M>,
        accumulated_interest: BigUint<M>,
        account_nonce: u64,
        timestamp: u64,
        index: BigUint<M>,
        entry_liquidation_threshold: BigUint<M>,
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
        }
    }

    pub fn get_total_amount(&self) -> BigUint<M> {
        &self.amount + &self.accumulated_interest
    }
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct AssetConfig<M: ManagedTypeApi> {
    // Basic parameters
    pub ltv: BigUint<M>,
    pub liquidation_threshold: BigUint<M>,
    pub liquidation_bonus: BigUint<M>,
    pub liquidation_base_fee: BigUint<M>,

    // Caps
    pub borrow_cap: Option<BigUint<M>>, // Maximum amount that can be borrowed across all users
    pub supply_cap: Option<BigUint<M>>, // Maximum amount that can be supplied across all users

    // Asset usage flags
    pub can_be_collateral: bool,
    pub can_be_borrowed: bool,

    // E-mode configuration
    pub is_e_mode_enabled: bool, // true if the asset has at least one e-mode category

    // Isolation mode
    pub is_isolated: bool,
    pub debt_ceiling_usd: BigUint<M>, // Max debt ceiling for this asset in isolation mode

    // Siloed borrowing
    pub is_siloed: bool,

    // Flashloan flag if the asset supports flashloans
    pub flashloan_enabled: bool,
    pub flash_loan_fee: BigUint<M>,

    // Isolation mode borrow flags (Usully for stablecoins)
    pub can_borrow_in_isolation: bool,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeCategory<M: ManagedTypeApi> {
    pub id: u8,
    pub ltv: BigUint<M>,
    pub liquidation_threshold: BigUint<M>,
    pub liquidation_bonus: BigUint<M>,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeAssetConfig {
    pub can_be_collateral: bool,
    pub can_be_borrowed: bool,
}

#[type_abi]
#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, Clone)]
pub struct NftAccountAttributes {
    pub is_isolated: bool,
    pub e_mode_category: u8,
    pub is_vault: bool,
}

#[type_abi]
#[derive(
    ManagedVecItem, TopDecode, TopEncode, NestedDecode, NestedEncode, Clone, PartialEq, Eq, Debug,
)]
pub struct EgldOrEsdtTokenPaymentNew<M: ManagedTypeApi> {
    pub token_identifier: EgldOrEsdtTokenIdentifier<M>,
    pub token_nonce: u64,
    pub amount: BigUint<M>,
}

#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum OracleOverrideType {
    None,
    Derived,
    Lp,
}

#[type_abi]
#[derive(
    ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, Clone, Eq, PartialEq,
)]
pub enum ExchangeSource {
    None,
    XExchange,
}

#[type_abi]
#[derive(ManagedVecItem, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct OracleOverride<M: ManagedTypeApi> {
    pub original_token_id: EgldOrEsdtTokenIdentifier<M>,
    pub contract_address: ManagedAddress<M>,
    pub token_type: OracleOverrideType,
}
