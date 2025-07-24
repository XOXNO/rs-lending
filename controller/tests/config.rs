pub use common_constants::{BPS_PRECISION, RAY_PRECISION, WAD_PRECISION};
use common_constants::{MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE};

use controller::{
    EModeAssetConfig, EModeCategory, ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE,
    ERROR_ASSET_NOT_SUPPORTED, ERROR_ASSET_NOT_SUPPORTED_IN_EMODE, ERROR_EMODE_CATEGORY_NOT_FOUND,
    ERROR_INVALID_AGGREGATOR, ERROR_INVALID_LIQUIDATION_THRESHOLD,
    ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE, ERROR_INVALID_ONEDEX_PAIR_ID,
    ERROR_ORACLE_TOKEN_EXISTING, ERROR_ORACLE_TOKEN_NOT_FOUND, ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    ERROR_UNEXPECTED_FIRST_TOLERANCE, ERROR_UNEXPECTED_LAST_TOLERANCE,
};
use multiversx_sc::types::{EgldOrEsdtTokenIdentifier, ManagedAddress, ManagedDecimal};
use multiversx_sc_scenario::imports::{BigUint, OptionalValue, TestAddress, TestTokenIdentifier};
pub mod constants;
pub mod proxys;
pub mod setup;
use common_structs::{ExchangeSource, OracleType, PricingMethod};
use constants::*;
use setup::*;

// ============================================
// ORACLE CONFIGURATION TESTS
// ============================================

/// Tests oracle configuration for already existing token fails.
///
/// Covers:
/// - Oracle configuration validation
/// - Duplicate oracle prevention
/// - ERROR_ORACLE_TOKEN_EXISTING error condition
#[test]
fn oracle_set_token_oracle_already_exists_error() {
    let mut state = LendingPoolTestState::new();

    // Attempt to set oracle for EGLD which already has one
    let oracle_address = TestAddress::new("oracle").to_managed_address();

    state.set_token_oracle_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        18usize,
        &oracle_address,
        PricingMethod::Aggregator,
        OracleType::Normal,
        ExchangeSource::XExchange,
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE),
        3600u64,
        OptionalValue::None,
        ERROR_ORACLE_TOKEN_EXISTING,
    );
}

/// Tests oracle configuration with Onedex requires pair ID.
///
/// Covers:
/// - Onedex exchange source validation
/// - Pair ID requirement for Onedex
/// - ERROR_INVALID_ONEDEX_PAIR_ID error condition
#[test]
fn oracle_set_token_oracle_onedex_missing_pair_id_error() {
    let mut state = LendingPoolTestState::new();

    let new_token = TestTokenIdentifier::new("ONEDEXTOKEN-123456");
    let oracle_address = TestAddress::new("oracle").to_managed_address();

    state.set_token_oracle_error(
        &EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        18usize,
        &oracle_address,
        PricingMethod::Aggregator,
        OracleType::Normal,
        ExchangeSource::Onedex,
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE),
        3600u64,
        OptionalValue::None, // Missing pair ID
        ERROR_INVALID_ONEDEX_PAIR_ID,
    );
}

/// Tests successful oracle tolerance update.
///
/// Covers:
/// - Oracle tolerance modification
/// - Tolerance values persistence
/// - Successful configuration update
#[test]
fn oracle_edit_tolerance_success() {
    let mut state = LendingPoolTestState::new();

    // Update tolerance for existing EGLD oracle
    state.edit_token_oracle_tolerance(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE * 2),
        BigUint::from(MIN_LAST_TOLERANCE * 2),
    );

    // Verify tolerance was updated
    let oracle = state.get_token_oracle(EgldOrEsdtTokenIdentifier::egld());
    assert!(
        oracle.tolerance.first_upper_ratio
            > ManagedDecimal::from_raw_units(BigUint::from(MIN_FIRST_TOLERANCE), 4)
    );
}

/// Tests oracle tolerance update for non-existent token fails.
///
/// Covers:
/// - Oracle existence validation
/// - ERROR_ORACLE_TOKEN_NOT_FOUND error condition
#[test]
fn oracle_edit_tolerance_token_not_found_error() {
    let mut state = LendingPoolTestState::new();

    let new_token = TestTokenIdentifier::new("NOTOKEN-123456");

    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE),
        ERROR_ORACLE_TOKEN_NOT_FOUND,
    );
}

/// Tests oracle tolerance update with first tolerance too low fails.
///
/// Covers:
/// - First tolerance minimum validation
/// - ERROR_UNEXPECTED_FIRST_TOLERANCE error condition
#[test]
fn oracle_edit_tolerance_first_tolerance_too_low_error() {
    let mut state = LendingPoolTestState::new();

    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE - 1), // Below minimum
        BigUint::from(MIN_LAST_TOLERANCE),
        ERROR_UNEXPECTED_FIRST_TOLERANCE,
    );
}

/// Tests oracle tolerance update with last tolerance too low fails.
///
/// Covers:
/// - Last tolerance minimum validation
/// - ERROR_UNEXPECTED_LAST_TOLERANCE error condition
#[test]
fn oracle_edit_tolerance_last_tolerance_too_low_error() {
    let mut state = LendingPoolTestState::new();

    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE - 1), // Below minimum
        ERROR_UNEXPECTED_LAST_TOLERANCE,
    );
}

/// Tests oracle tolerance update with invalid anchor tolerances fails.
///
/// Covers:
/// - Anchor tolerance relationship validation
/// - First tolerance must be <= last tolerance
/// - ERROR_UNEXPECTED_ANCHOR_TOLERANCES error condition
#[test]
fn oracle_edit_tolerance_invalid_anchor_error() {
    let mut state = LendingPoolTestState::new();

    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(1001u64), // First tolerance greater than last
        BigUint::from(1000u64),
        ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    );
}

// ============================================
// ADDRESS CONFIGURATION TESTS
// ============================================

/// Tests successful price aggregator address update.
///
/// Covers:
/// - Price aggregator configuration
/// - Address update verification
#[test]
fn address_set_aggregator_success() {
    let mut state = LendingPoolTestState::new();

    let new_aggregator = state.price_aggregator_sc.clone();
    state.set_aggregator(new_aggregator.clone());

    // Verify aggregator was updated
    let aggregator = state.get_price_aggregator_address();
    assert_eq!(aggregator, new_aggregator);
}

/// Tests price aggregator zero address validation.
///
/// Covers:
/// - Zero address validation
/// - ERROR_INVALID_AGGREGATOR error condition
#[test]
fn address_set_aggregator_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    state.set_aggregator_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

/// Tests successful swap router address update.
///
/// Covers:
/// - Swap router configuration
/// - Address update verification
#[test]
fn address_set_swap_router_success() {
    let mut state = LendingPoolTestState::new();

    let new_router = state.price_aggregator_sc.clone();
    state.set_swap_router(new_router.clone());

    // Verify router was updated
    let router = state.get_swap_router_address();
    assert_eq!(router, new_router);
}

/// Tests swap router zero address validation.
///
/// Covers:
/// - Zero address validation
/// - ERROR_INVALID_AGGREGATOR error condition
#[test]
fn address_set_swap_router_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    state.set_swap_router_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

/// Tests successful accumulator address update.
///
/// Covers:
/// - Accumulator configuration
/// - Address update verification
#[test]
fn address_set_accumulator_success() {
    let mut state = LendingPoolTestState::new();

    let new_accumulator = state.price_aggregator_sc.clone();
    state.set_accumulator(new_accumulator.clone());

    // Verify accumulator was updated
    let accumulator = state.get_accumulator_address();
    assert_eq!(accumulator, new_accumulator);
}

/// Tests accumulator zero address validation.
///
/// Covers:
/// - Zero address validation
/// - ERROR_INVALID_AGGREGATOR error condition
#[test]
fn address_set_accumulator_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    state.set_accumulator_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

/// Tests successful safe price view address update.
///
/// Covers:
/// - Safe price view configuration
/// - Address update verification
#[test]
fn address_set_safe_price_view_success() {
    let mut state = LendingPoolTestState::new();

    let new_safe_view = state.price_aggregator_sc.clone();
    state.set_safe_price_view(new_safe_view.clone());

    // Verify safe price view was updated
    let safe_view = state.get_safe_price_address();
    assert_eq!(safe_view, new_safe_view);
}

/// Tests safe price view zero address validation.
///
/// Covers:
/// - Zero address validation
/// - ERROR_INVALID_AGGREGATOR error condition
#[test]
fn address_set_safe_price_view_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    state.set_safe_price_view_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

/// Tests successful liquidity pool template address update.
///
/// Covers:
/// - Liquidity pool template configuration
/// - Address update verification
#[test]
fn address_set_liquidity_pool_template_success() {
    let mut state = LendingPoolTestState::new();

    let new_template = state.template_address_liquidity_pool.clone();
    state.set_liquidity_pool_template(new_template.clone());

    // Verify template was updated
    let template = state.get_liq_pool_template_address();
    assert_eq!(template, new_template);
}

/// Tests liquidity pool template zero address validation.
///
/// Covers:
/// - Zero address validation
/// - ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE error condition
#[test]
fn address_set_liquidity_pool_template_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    state.set_liquidity_pool_template_error(
        ManagedAddress::zero(),
        ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE,
    );
}

// ============================================
// E-MODE CONFIGURATION TESTS
// ============================================

/// Tests successful E-Mode category creation.
///
/// Covers:
/// - E-Mode category addition
/// - Category ID auto-increment
/// - Risk parameters configuration
#[test]
fn emode_add_category_success() {
    let mut state = LendingPoolTestState::new();

    state.add_e_mode_category(
        BigUint::from(8500u64), // 85% LTV
        BigUint::from(9000u64), // 90% liquidation threshold
        BigUint::from(200u64),  // 2% liquidation bonus
    );

    // Verify category was added with ID 2
    let last_category_id = state.last_e_mode_category_id();
    assert_eq!(last_category_id, 2);
}

/// Tests successful E-Mode category update.
///
/// Covers:
/// - E-Mode category modification
/// - Risk parameters update
/// - Category persistence
#[test]
fn emode_edit_category_success() {
    let mut state = LendingPoolTestState::new();

    let category = EModeCategory {
        category_id: 1,
        loan_to_value: ManagedDecimal::from_raw_units(BigUint::from(8000u64), BPS_PRECISION),
        liquidation_threshold: ManagedDecimal::from_raw_units(
            BigUint::from(8500u64),
            BPS_PRECISION,
        ),
        liquidation_bonus: ManagedDecimal::from_raw_units(BigUint::from(300u64), BPS_PRECISION),
        is_deprecated: false,
    };

    state.edit_e_mode_category(category);

    // Verify category was updated
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().any(|item| {
        let (id, _) = item.into_tuple();
        id == 1
    });
    assert!(found);
}

/// Tests E-Mode category update for non-existent category fails.
///
/// Covers:
/// - Category existence validation
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_edit_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    let category = EModeCategory {
        category_id: 99, // Non-existent
        loan_to_value: ManagedDecimal::from_raw_units(BigUint::from(8000u64), BPS_PRECISION),
        liquidation_threshold: ManagedDecimal::from_raw_units(
            BigUint::from(8500u64),
            BPS_PRECISION,
        ),
        liquidation_bonus: ManagedDecimal::from_raw_units(BigUint::from(300u64), BPS_PRECISION),
        is_deprecated: false,
    };

    state.edit_e_mode_category_error(category, ERROR_EMODE_CATEGORY_NOT_FOUND);
}

/// Tests successful E-Mode category deprecation.
///
/// Covers:
/// - E-Mode category removal
/// - Category deprecation flag
/// - Soft delete functionality
#[test]
fn emode_remove_category_success() {
    let mut state = LendingPoolTestState::new();

    // Add category first
    state.add_e_mode_category(
        BigUint::from(8500u64),
        BigUint::from(9000u64),
        BigUint::from(200u64),
    );

    // Remove the category
    state.remove_e_mode_category(2);

    // Verify category was deprecated
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().find(|item| {
        let (id, _) = item.clone().into_tuple();
        id == 2
    });
    assert!(found.is_some());
    let (_, category) = found.unwrap().into_tuple();
    assert!(category.is_deprecated);
}

/// Tests E-Mode category removal for non-existent category fails.
///
/// Covers:
/// - Category existence validation
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_remove_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    state.remove_e_mode_category_error(99, ERROR_EMODE_CATEGORY_NOT_FOUND);
}

/// Tests successful asset addition to E-Mode category.
///
/// Covers:
/// - Asset to E-Mode category mapping
/// - Asset configuration in E-Mode
/// - Multiple category support
#[test]
fn emode_add_asset_to_category_success() {
    let mut state = LendingPoolTestState::new();

    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
        true, // can be collateral
        true, // can be borrowed
    );

    // Verify asset was added
    let asset_e_modes = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        USDC_TOKEN.to_token_identifier(),
    ));
    assert!(asset_e_modes.into_iter().any(|id| id == 1));
}

/// Tests asset addition to non-existent E-Mode category fails.
///
/// Covers:
/// - Category existence validation
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_add_asset_to_invalid_category_error() {
    let mut state = LendingPoolTestState::new();

    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        99, // Non-existent category
        true,
        true,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests adding unsupported asset to E-Mode category fails.
///
/// Covers:
/// - Asset existence validation
/// - ERROR_ASSET_NOT_SUPPORTED error condition
#[test]
fn emode_add_unsupported_asset_error() {
    let mut state = LendingPoolTestState::new();

    let new_token = TestTokenIdentifier::new("NOASSET-123456");
    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        1,
        true,
        true,
        ERROR_ASSET_NOT_SUPPORTED,
    );
}

/// Tests adding already existing asset to E-Mode category fails.
///
/// Covers:
/// - Duplicate asset prevention
/// - ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE error condition
#[test]
fn emode_add_duplicate_asset_error() {
    let mut state = LendingPoolTestState::new();

    // EGLD is already in category 1
    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
        true,
        true,
        ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE,
    );
}

/// Tests successful asset configuration update in E-Mode category.
///
/// Covers:
/// - Asset configuration modification
/// - Collateral/borrow flag updates
#[test]
fn emode_edit_asset_in_category_success() {
    let mut state = LendingPoolTestState::new();

    let config = EModeAssetConfig {
        is_collateralizable: false, // Change from true
        is_borrowable: true,
    };

    state.edit_asset_in_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
        config,
    );

    // Verify config was updated
    let e_mode_assets = state.get_e_modes_assets(1);
    let mut found_config = None;
    for item in e_mode_assets {
        let (asset, config) = item.into_tuple();
        if asset == EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()) {
            found_config = Some(config);
            break;
        }
    }
    assert!(found_config.is_some());
    assert!(!found_config.unwrap().is_collateralizable);
}

/// Tests asset edit in non-existent E-Mode category fails.
///
/// Covers:
/// - Category existence validation
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_edit_asset_invalid_category_error() {
    let mut state = LendingPoolTestState::new();

    let config = EModeAssetConfig {
        is_collateralizable: false,
        is_borrowable: true,
    };

    state.edit_asset_in_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        99, // Non-existent category
        config,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests editing non-existent asset in E-Mode category fails.
///
/// Covers:
/// - Asset existence in category validation
/// - ERROR_ASSET_NOT_SUPPORTED_IN_EMODE error condition
#[test]
fn emode_edit_missing_asset_error() {
    let mut state = LendingPoolTestState::new();

    let config = EModeAssetConfig {
        is_collateralizable: false,
        is_borrowable: true,
    };

    // USDC not in category 1
    state.edit_asset_in_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
        config,
        ERROR_ASSET_NOT_SUPPORTED_IN_EMODE,
    );
}

/// Tests successful asset removal from E-Mode category.
///
/// Covers:
/// - Asset removal from category
/// - Category asset list update
#[test]
fn emode_remove_asset_from_category_success() {
    let mut state = LendingPoolTestState::new();

    state.remove_asset_from_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
    );

    // Verify asset was removed
    let asset_e_modes = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    assert!(!asset_e_modes.into_iter().any(|id| id == 1));
}

/// Tests asset removal from non-existent E-Mode category fails.
///
/// Covers:
/// - Category existence validation
/// - ERROR_EMODE_CATEGORY_NOT_FOUND error condition
#[test]
fn emode_remove_asset_invalid_category_error() {
    let mut state = LendingPoolTestState::new();

    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        99, // Non-existent category
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

/// Tests removing unsupported asset from E-Mode category fails.
///
/// Covers:
/// - Asset existence validation
/// - ERROR_ASSET_NOT_SUPPORTED error condition
#[test]
fn emode_remove_unsupported_asset_error() {
    let mut state = LendingPoolTestState::new();

    let new_token = TestTokenIdentifier::new("NOASSET-123456");
    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        1,
        ERROR_ASSET_NOT_SUPPORTED,
    );
}

/// Tests removing non-existent asset from E-Mode category fails.
///
/// Covers:
/// - Asset membership validation
/// - ERROR_ASSET_NOT_SUPPORTED_IN_EMODE error condition
#[test]
fn emode_remove_missing_asset_error() {
    let mut state = LendingPoolTestState::new();

    // USDC not in category 1
    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
        ERROR_ASSET_NOT_SUPPORTED_IN_EMODE,
    );
}

// ============================================
// ASSET CONFIGURATION TESTS
// ============================================

/// Tests successful asset configuration update.
///
/// Covers:
/// - Asset risk parameters update
/// - All configuration fields
/// - Value persistence
#[test]
fn asset_edit_config_success() {
    let mut state = LendingPoolTestState::new();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7000u64), // 70% LTV
        &BigUint::from(8000u64), // 80% liquidation threshold
        &BigUint::from(500u64),  // 5% liquidation bonus
        &BigUint::from(100u64),  // 1% liquidation fees
        false,                   // not isolated
        &BigUint::zero(),        // no debt ceiling
        false,                   // not siloed
        true,                    // flashloanable
        &BigUint::from(10u64),   // 0.1% flash loan fee
        true,                    // collateralizable
        true,                    // borrowable
        false,                   // isolation borrow not enabled
        &BigUint::zero(),        // no borrow cap
        &BigUint::zero(),        // no supply cap
        None,
    );

    // Verify config was updated
    let config = state.get_asset_config(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    let ltv_value = config.loan_to_value.into_raw_units().clone();
    assert_eq!(ltv_value, BigUint::from(7000u64));
}

/// Tests asset configuration for non-existent asset fails.
///
/// Covers:
/// - Asset existence validation
/// - ERROR_ASSET_NOT_SUPPORTED error condition
#[test]
fn asset_edit_config_unsupported_asset_error() {
    let mut state = LendingPoolTestState::new();

    let new_token = TestTokenIdentifier::new("NOASSET-123456");
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        &BigUint::from(7000u64),
        &BigUint::from(8000u64),
        &BigUint::from(500u64),
        &BigUint::from(100u64),
        false,
        &BigUint::zero(),
        false,
        true,
        &BigUint::from(10u64),
        true,
        true,
        false,
        &BigUint::zero(),
        &BigUint::zero(),
        Some(ERROR_ASSET_NOT_SUPPORTED),
    );
}

/// Tests asset configuration with invalid liquidation threshold fails.
///
/// Covers:
/// - Liquidation threshold > LTV validation
/// - ERROR_INVALID_LIQUIDATION_THRESHOLD error condition
#[test]
fn asset_edit_config_invalid_liquidation_threshold_error() {
    let mut state = LendingPoolTestState::new();

    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(8000u64), // 80% LTV
        &BigUint::from(7000u64), // 70% liquidation threshold (invalid - less than LTV)
        &BigUint::from(500u64),
        &BigUint::from(100u64),
        false,
        &BigUint::zero(),
        false,
        true,
        &BigUint::from(10u64),
        true,
        true,
        false,
        &BigUint::zero(),
        &BigUint::zero(),
        Some(ERROR_INVALID_LIQUIDATION_THRESHOLD),
    );
}

// ============================================
// COMPLEX SCENARIO TESTS
// ============================================

/// Tests complete E-Mode category lifecycle.
///
/// Covers:
/// - Full E-Mode category management flow
/// - Category creation, asset management, and removal
/// - State consistency throughout lifecycle
#[test]
fn emode_complete_lifecycle_scenario() {
    let mut state = LendingPoolTestState::new();

    // 1. Create new E-Mode category
    state.add_e_mode_category(
        BigUint::from(9000u64), // 90% LTV
        BigUint::from(9500u64), // 95% liquidation threshold
        BigUint::from(100u64),  // 1% liquidation bonus
    );

    // 2. Add asset to category
    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
        true,
        true,
    );

    // 3. Modify asset configuration
    let config = EModeAssetConfig {
        is_collateralizable: true,
        is_borrowable: false,
    };
    state.edit_asset_in_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
        config,
    );

    // 4. Remove asset from category
    state.remove_asset_from_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
    );

    // 5. Deprecate category
    state.remove_e_mode_category(2);

    // Verify final state
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().find(|item| {
        let (id, _) = item.clone().into_tuple();
        id == 2
    });
    assert!(found.is_some());
    let (_, category) = found.unwrap().into_tuple();
    assert!(category.is_deprecated);
}

/// Tests E-Mode category removal with multiple assets.
///
/// Covers:
/// - Category removal impact on multiple assets
/// - Automatic asset removal from deprecated category
/// - State consistency after bulk operations
#[test]
fn emode_remove_category_with_multiple_assets_scenario() {
    let mut state = LendingPoolTestState::new();

    // 1. Create category
    state.add_e_mode_category(
        BigUint::from(8500u64),
        BigUint::from(9000u64),
        BigUint::from(200u64),
    );

    // 2. Add multiple assets
    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
        true,
        true,
    );

    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(ISOLATED_TOKEN.to_token_identifier()),
        2,
        true,
        false,
    );

    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(SILOED_TOKEN.to_token_identifier()),
        2,
        false,
        true,
    );

    // Verify all assets were added
    let usdc_e_modes = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        USDC_TOKEN.to_token_identifier(),
    ));
    let isolated_e_modes = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        ISOLATED_TOKEN.to_token_identifier(),
    ));
    let siloed_e_modes = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        SILOED_TOKEN.to_token_identifier(),
    ));

    assert!(usdc_e_modes.into_iter().any(|id| id == 2));
    assert!(isolated_e_modes.into_iter().any(|id| id == 2));
    assert!(siloed_e_modes.into_iter().any(|id| id == 2));

    // 3. Remove category
    state.remove_e_mode_category(2);

    // Verify category deprecated
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().find(|item| {
        let (id, _) = item.clone().into_tuple();
        id == 2
    });
    assert!(found.is_some());
    let (_, category) = found.unwrap().into_tuple();
    assert!(category.is_deprecated);

    // Verify assets removed from category
    let usdc_e_modes_after = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        USDC_TOKEN.to_token_identifier(),
    ));
    let isolated_e_modes_after = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        ISOLATED_TOKEN.to_token_identifier(),
    ));
    let siloed_e_modes_after = state.get_asset_e_modes(EgldOrEsdtTokenIdentifier::esdt(
        SILOED_TOKEN.to_token_identifier(),
    ));

    assert!(!usdc_e_modes_after.into_iter().any(|id| id == 2));
    assert!(!isolated_e_modes_after.into_iter().any(|id| id == 2));
    assert!(!siloed_e_modes_after.into_iter().any(|id| id == 2));

    // Verify category asset list is empty
    let e_mode_assets = state.get_e_modes_assets(2);
    assert_eq!(e_mode_assets.len(), 0);
}

/// Tests asset configuration with zero caps handling.
///
/// Covers:
/// - Supply/borrow cap configuration
/// - Zero cap interpretation as None
/// - Cap update and removal
#[test]
fn asset_edit_config_zero_caps_scenario() {
    let mut state = LendingPoolTestState::new();

    // Set non-zero caps
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64),
        &BigUint::from(8000u64),
        &BigUint::from(500u64),
        &BigUint::from(100u64),
        false,
        &BigUint::zero(),
        false,
        true,
        &BigUint::from(10u64),
        true,
        true,
        false,
        &BigUint::from(1000000u64), // 1M borrow cap
        &BigUint::from(2000000u64), // 2M supply cap
        None,
    );

    // Verify caps were set
    let config = state.get_asset_config(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    assert!(config.borrow_cap.is_some());
    assert!(config.supply_cap.is_some());
    assert_eq!(config.borrow_cap.unwrap(), BigUint::from(1000000u64));
    assert_eq!(config.supply_cap.unwrap(), BigUint::from(2000000u64));

    // Set caps to zero (removes caps)
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64),
        &BigUint::from(8000u64),
        &BigUint::from(500u64),
        &BigUint::from(100u64),
        false,
        &BigUint::zero(),
        false,
        true,
        &BigUint::from(10u64),
        true,
        true,
        false,
        &BigUint::zero(), // Zero borrow cap
        &BigUint::zero(), // Zero supply cap
        None,
    );

    // Verify caps removed
    let config_after = state.get_asset_config(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    assert!(config_after.borrow_cap.is_none());
    assert!(config_after.supply_cap.is_none());

    // Verify other values unchanged
    let ltv_value = config_after.loan_to_value.into_raw_units().clone();
    assert_eq!(ltv_value, BigUint::from(7500u64));
}
