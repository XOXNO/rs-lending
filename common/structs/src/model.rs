#![no_std]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub const BP: u64 = 1_000_000_000; // Represents 100%
pub const MAX_BONUS: u64 = 300_000_000; // Represents 30% basis points
pub const MAX_THRESHOLD: u64 = BP / 2;

#[derive(TopEncode, TopDecode, TypeAbi)]
pub struct PoolParams<M: ManagedTypeApi> {
    pub r_max: BigUint<M>,
    pub r_base: BigUint<M>,
    pub r_slope1: BigUint<M>,
    pub r_slope2: BigUint<M>,
    pub u_optimal: BigUint<M>,
    pub reserve_factor: BigUint<M>,
}

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub enum UpdateAssetParamsType {
    None,
    LTV,
    LiquidationBonus,
    LiquidationThreshold,
}

#[derive(ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub enum AccountPositionType {
    None,
    Deposit,
    Borrow,
}

#[derive(ManagedVecItem, NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub struct AccountPosition<M: ManagedTypeApi> {
    pub deposit_type: AccountPositionType,
    pub account_nonce: u64,
    pub token_id: EgldOrEsdtTokenIdentifier<M>,
    pub amount: BigUint<M>,
    pub round: u64,
    pub index: BigUint<M>,

    pub entry_ltv: BigUint<M>,
    pub entry_liquidation_threshold: BigUint<M>,
    pub entry_liquidation_bonus: BigUint<M>,
}

impl<M: ManagedTypeApi> AccountPosition<M> {
    pub fn new(
        deposit_type: AccountPositionType,
        token_id: EgldOrEsdtTokenIdentifier<M>,
        amount: BigUint<M>,
        account_nonce: u64,
        round: u64,
        index: BigUint<M>,
        entry_ltv: BigUint<M>,
        entry_liquidation_threshold: BigUint<M>,
        entry_liquidation_bonus: BigUint<M>,
    ) -> Self {
        AccountPosition {
            deposit_type,
            token_id,
            amount,
            account_nonce,
            round,
            index,
            entry_ltv,
            entry_liquidation_threshold,
            entry_liquidation_bonus,
        }
    }
}

#[derive(ManagedVecItem, TypeAbi, TopEncode, TopDecode, NestedEncode, NestedDecode)]
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

    // Isolation mode borrow flags (Usully for stablecoins)
    pub can_borrow_in_isolation: bool,
}

#[derive(ManagedVecItem, TypeAbi, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeCategory<M: ManagedTypeApi> {
    pub id: u8,
    pub ltv: BigUint<M>,
    pub liquidation_threshold: BigUint<M>,
    pub liquidation_bonus: BigUint<M>,
}

#[derive(ManagedVecItem, TypeAbi, TopEncode, TopDecode, NestedEncode, NestedDecode)]
pub struct EModeAssetConfig {
    pub can_be_collateral: bool,
    pub can_be_borrowed: bool,
}

#[derive(NestedEncode, NestedDecode, TopEncode, TopDecode, TypeAbi, Clone)]
pub struct NftAccountAttributes {
    pub is_isolated: bool,
    pub e_mode_category: u8,
}

#[derive(
    ManagedVecItem, TopDecode, TopEncode, NestedDecode, NestedEncode, TypeAbi, Clone, PartialEq, Eq, Debug,
)]
pub struct EgldOrEsdtTokenPaymentNew<M: ManagedTypeApi> {
    pub token_identifier: EgldOrEsdtTokenIdentifier<M>,
    pub token_nonce: u64,
    pub amount: BigUint<M>,
}