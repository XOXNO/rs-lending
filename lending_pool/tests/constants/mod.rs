use lending_pool::AssetConfig;
use multiversx_sc::types::{BigUint, TestAddress, TestSCAddress};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{MxscPath, TestTokenIdentifier},
};
use std::ops::Mul;

pub const SECONDS_PER_YEAR: u64 = 31_556_926;
pub const SECONDS_PER_DAY: u64 = 86_400; // 24 * 60 * 60

pub const DOLLAR_TICKER: &[u8] = b"USD";

pub const LTV: u64 = 750_000_000; // 75%
pub const E_MODE_LTV: u64 = 800_000_000; // 80%

pub const R_BASE: u64 = 20_000_000; // 2%
pub const R_MAX: u64 = 1_000_000_000; // 100%
pub const R_SLOPE1: u64 = 100_000_000; // 10%
pub const R_SLOPE2: u64 = 1_000_000_000; // 100%
pub const U_OPTIMAL: u64 = 800_000_000; // 80%
pub const RESERVE_FACTOR: u64 = 300_000_000; // 30%
pub const LIQ_THRESOLD: u64 = 800_000_000; // 80%
pub const E_MODE_LIQ_THRESOLD: u64 = 850_000_000; // 85%
pub const LIQ_BONUS: u64 = 100_000_000; // 10%
pub const E_MODE_LIQ_BONUS: u64 = 50_000_000; // 5%

pub const LIQ_BASE_FEE: u64 = 50_000_000; // 5%
pub const MAX_BORROW: u64 = 6_000_000_000;
pub const FLASH_LOAN_FEE: u64 = 5_000_000; // 0.5%
pub const DECIMALS: u64 = 1_000_000_000;

pub const ACCOUNT_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("ACC-abcdef");

pub const EGLD_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("WEGLD-abcdef");
pub const EGLD_TICKER: &[u8] = b"WEGLD";
pub const EGLD_PRICE_IN_DOLLARS: u64 = 40; // $40
pub const EGLD_DECIMALS: u64 = 18;

pub const SEGLD_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("SEGLD-abcdef");
pub const SEGLD_TICKER: &[u8] = b"SEGLD";
pub const SEGLD_PRICE_IN_DOLLARS: u64 = 50; // $50
pub const SEGLD_DECIMALS: u64 = 18;

pub const LEGLD_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("LEGLD-abcdef");
pub const LEGLD_TICKER: &[u8] = b"LEGLD";
pub const LEGLD_PRICE_IN_DOLLARS: u64 = 50; // $50
pub const LEGLD_DECIMALS: u64 = 18;

pub const USDC_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("USDC-abcdef");
pub const USDC_TICKER: &[u8] = b"USDC";
pub const USDC_PRICE_IN_DOLLARS: u64 = 1; // $1
pub const USDC_DECIMALS: u64 = 6;

pub const XEGLD_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("XEGLD-abcdef");
pub const XEGLD_TICKER: &[u8] = b"XEGLD";
pub const XEGLD_PRICE_IN_DOLLARS: u64 = 50; // $50
pub const XEGLD_DECIMALS: u64 = 18;

pub const ISOLATED_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("ISOLATED-abcdef");
pub const ISOLATED_TICKER: &[u8] = b"ISOLATED";
pub const ISOLATED_PRICE_IN_DOLLARS: u64 = 5; // $5
pub const ISOLATED_DECIMALS: u64 = 18;

pub const SILOED_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("SILOED-abcdef");
pub const SILOED_TICKER: &[u8] = b"SILOED";
pub const SILOED_PRICE_IN_DOLLARS: u64 = 4; // $4
pub const SILOED_DECIMALS: u64 = 18;

pub const CAPPED_TOKEN: TestTokenIdentifier = TestTokenIdentifier::new("CAPPED-abcdef");
pub const CAPPED_TICKER: &[u8] = b"CAPPED";
pub const CAPPED_PRICE_IN_DOLLARS: u64 = 2; // $2
pub const CAPPED_DECIMALS: u64 = 8;

pub const LENDING_POOL_ADDRESS: TestSCAddress = TestSCAddress::new("lending-pool");
pub const LIQUIDITY_POOL_ADDRESS: TestSCAddress = TestSCAddress::new("liquidity-pool");
pub const PRICE_AGGREGATOR_ADDRESS: TestSCAddress = TestSCAddress::new("price-aggregator");

pub const OWNER_ADDRESS: TestAddress = TestAddress::new("owner");

pub const ORACLE_ADDRESS_1: TestAddress = TestAddress::new("oracle1");
pub const ORACLE_ADDRESS_2: TestAddress = TestAddress::new("oracle2");
pub const ORACLE_ADDRESS_3: TestAddress = TestAddress::new("oracle3");
pub const ORACLE_ADDRESS_4: TestAddress = TestAddress::new("oracle4");

pub const LENDING_POOL_PATH: MxscPath = MxscPath::new("../output/lending-pool.mxsc.json");
pub const LIQUIDITY_POOL_PATH: MxscPath =
    MxscPath::new("../liquidity_pool/output/liquidity-pool.mxsc.json");
pub const PRICE_AGGREGATOR_PATH: MxscPath =
    MxscPath::new("../price-aggregator/output/price-aggregator.mxsc.json");

pub struct SetupConfig {
    // Basic parameters
    pub config: AssetConfig<StaticApi>,
    pub r_max: u64,
    pub r_base: u64,
    pub r_slope1: u64,
    pub r_slope2: u64,
    pub u_optimal: u64,
    pub reserve_factor: u64,
}

pub fn get_usdc_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: Option::None,
            supply_cap: Option::None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: true,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_egld_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_xegld_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_segld_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_legld_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_isolated_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: true,
            debt_ceiling_usd: BigUint::from(10_000_000u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_siloed_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: None,
            supply_cap: None,
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(1000u64).mul(DECIMALS), // 1000 USD
            is_siloed: true,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}

pub fn get_capped_config() -> SetupConfig {
    SetupConfig {
        config: AssetConfig {
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(LIQ_BONUS),
            liquidation_base_fee: BigUint::from(LIQ_BASE_FEE),
            borrow_cap: Some(BigUint::from(50_000_000u64)),
            supply_cap: Some(BigUint::from(100_000_000u64)),
            can_be_collateral: true,
            can_be_borrowed: true,
            is_e_mode_enabled: false,
            is_isolated: false,
            debt_ceiling_usd: BigUint::from(0u64),
            is_siloed: false,
            flashloan_enabled: true,
            flash_loan_fee: BigUint::from(FLASH_LOAN_FEE),
            can_borrow_in_isolation: false,
        },
        r_max: R_MAX,
        r_base: R_BASE,
        r_slope1: R_SLOPE1,
        r_slope2: R_SLOPE2,
        u_optimal: U_OPTIMAL,
        reserve_factor: RESERVE_FACTOR,
    }
}
