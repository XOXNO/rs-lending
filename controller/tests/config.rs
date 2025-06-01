use common_constants::{MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE};
use controller::{
    EModeAssetConfig, EModeCategory, BPS_PRECISION, ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE,
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

#[test]
fn test_set_token_oracle_already_exists_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set oracle for a token that already has one (EGLD)
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

#[test]
fn test_set_token_oracle_onedex_missing_pair_id_error() {
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
        OptionalValue::None,
        ERROR_INVALID_ONEDEX_PAIR_ID,
    );
}

#[test]
fn test_edit_token_oracle_tolerance_success() {
    let mut state = LendingPoolTestState::new();

    // Edit tolerance for existing oracle (EGLD)
    state.edit_token_oracle_tolerance(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE * 2),
        BigUint::from(MIN_LAST_TOLERANCE * 2),
    );

    // Verify tolerance was updated
    let oracle = state.get_token_oracle(EgldOrEsdtTokenIdentifier::egld());
    // Check that the tolerance values were updated (comparing with the original MIN values)
    assert!(
        oracle.tolerance.first_upper_ratio
            > ManagedDecimal::from_raw_units(BigUint::from(MIN_FIRST_TOLERANCE), 4)
    );
}

#[test]
fn test_edit_token_oracle_tolerance_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit tolerance for non-existent oracle
    let new_token = TestTokenIdentifier::new("NOTOKEN-123456");

    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE),
        ERROR_ORACLE_TOKEN_NOT_FOUND,
    );
}
#[test]
fn test_edit_token_oracle_tolerance_first_tolerance_too_low_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit tolerance for non-existent oracle
    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE - 1),
        BigUint::from(MIN_LAST_TOLERANCE),
        ERROR_UNEXPECTED_FIRST_TOLERANCE,
    );
}
#[test]
fn test_edit_token_oracle_tolerance_last_tolerance_too_low_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit tolerance for non-existent oracle
    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(MIN_FIRST_TOLERANCE),
        BigUint::from(MIN_LAST_TOLERANCE - 1),
        ERROR_UNEXPECTED_LAST_TOLERANCE,
    );
}

#[test]
fn test_edit_token_oracle_tolerance_anchor_tolerances_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit tolerance for non-existent oracle
    state.edit_token_oracle_tolerance_error(
        &EgldOrEsdtTokenIdentifier::egld(),
        BigUint::from(1001u64),
        BigUint::from(1000u64),
        ERROR_UNEXPECTED_ANCHOR_TOLERANCES,
    );
}

// ============================================
// ADDRESS CONFIGURATION TESTS
// ============================================

#[test]
fn test_set_aggregator_success() {
    let mut state = LendingPoolTestState::new();

    // Set a new aggregator address
    let new_aggregator = state.price_aggregator_sc.clone();
    state.set_aggregator(new_aggregator.clone());

    // Verify aggregator was set
    let aggregator = state.get_price_aggregator_address();
    assert_eq!(aggregator, new_aggregator);
}

#[test]
fn test_set_aggregator_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set zero address
    state.set_aggregator_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

#[test]
fn test_set_swap_router_success() {
    let mut state = LendingPoolTestState::new();

    // Set a new swap router address
    let new_router = state.price_aggregator_sc.clone(); // Using an existing SC address
    state.set_swap_router(new_router.clone());

    // Verify swap router was set
    let router = state.get_swap_router_address();
    assert_eq!(router, new_router);
}

#[test]
fn test_set_swap_router_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set zero address
    state.set_swap_router_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

#[test]
fn test_set_accumulator_success() {
    let mut state = LendingPoolTestState::new();

    // Set a new accumulator address
    let new_accumulator = state.price_aggregator_sc.clone(); // Using an existing SC address
    state.set_accumulator(new_accumulator.clone());

    // Verify accumulator was set
    let accumulator = state.get_accumulator_address();
    assert_eq!(accumulator, new_accumulator);
}

#[test]
fn test_set_accumulator_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set zero address
    state.set_accumulator_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

#[test]
fn test_set_safe_price_view_success() {
    let mut state = LendingPoolTestState::new();

    // Set a new safe price view address
    let new_safe_view = state.price_aggregator_sc.clone(); // Using an existing SC address
    state.set_safe_price_view(new_safe_view.clone());

    // Verify safe price view was set
    let safe_view = state.get_safe_price_address();
    assert_eq!(safe_view, new_safe_view);
}

#[test]
fn test_set_safe_price_view_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set zero address
    state.set_safe_price_view_error(ManagedAddress::zero(), ERROR_INVALID_AGGREGATOR);
}

#[test]
fn test_set_liquidity_pool_template_success() {
    let mut state = LendingPoolTestState::new();

    // Set a new liquidity pool template address
    let new_template = state.template_address_liquidity_pool.clone();
    state.set_liquidity_pool_template(new_template.clone());

    // Verify template was set
    let template = state.get_liq_pool_template_address();
    assert_eq!(template, new_template);
}

#[test]
fn test_set_liquidity_pool_template_zero_address_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set zero address
    state.set_liquidity_pool_template_error(
        ManagedAddress::zero(),
        ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE,
    );
}

// ============================================
// E-MODE CONFIGURATION TESTS
// ============================================

#[test]
fn test_add_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Add a new e-mode category
    state.add_e_mode_category(
        BigUint::from(8500u64), // 85% LTV
        BigUint::from(9000u64), // 90% liquidation threshold
        BigUint::from(200u64),  // 2% liquidation bonus
    );

    // Verify category was added
    let last_category_id = state.last_e_mode_category_id();
    assert_eq!(last_category_id, 2); // Should be 2 since we already have category 1
}

#[test]
fn test_edit_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Edit existing e-mode category
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

#[test]
fn test_edit_e_mode_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit non-existent category
    let category = EModeCategory {
        category_id: 99,
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

#[test]
fn test_remove_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Add a new category first
    state.add_e_mode_category(
        BigUint::from(8500u64),
        BigUint::from(9000u64),
        BigUint::from(200u64),
    );

    // Remove the category
    state.remove_e_mode_category(2);

    // Verify category was marked as deprecated
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().find(|item| {
        let (id, _) = item.clone().into_tuple();
        id == 2
    });
    assert!(found.is_some());
    let (_, category) = found.unwrap().into_tuple();
    assert!(category.is_deprecated);
}

#[test]
fn test_remove_e_mode_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to remove non-existent category
    state.remove_e_mode_category_error(99, ERROR_EMODE_CATEGORY_NOT_FOUND);
}

#[test]
fn test_add_asset_to_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Add USDC to e-mode category 1
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

#[test]
fn test_add_asset_to_e_mode_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to add asset to non-existent category
    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        99,
        true,
        true,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

#[test]
fn test_add_asset_to_e_mode_category_asset_not_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Try to add non-existent asset
    let new_token = TestTokenIdentifier::new("NOASSET-123456");
    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        1,
        true,
        true,
        ERROR_ASSET_NOT_SUPPORTED,
    );
}

#[test]
fn test_add_asset_to_e_mode_category_already_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Try to add EGLD which is already in category 1
    state.add_asset_to_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        1,
        true,
        true,
        ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE,
    );
}

#[test]
fn test_edit_asset_in_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Edit EGLD in e-mode category 1
    let config = EModeAssetConfig {
        is_collateralizable: false,
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

#[test]
fn test_edit_asset_in_e_mode_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit asset in non-existent category
    let config = EModeAssetConfig {
        is_collateralizable: false,
        is_borrowable: true,
    };

    state.edit_asset_in_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        99,
        config,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

#[test]
fn test_edit_asset_in_e_mode_category_asset_not_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit USDC which is not in category 1
    let config = EModeAssetConfig {
        is_collateralizable: false,
        is_borrowable: true,
    };

    state.edit_asset_in_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
        config,
        ERROR_ASSET_NOT_SUPPORTED_IN_EMODE,
    );
}

#[test]
fn test_remove_asset_from_e_mode_category_success() {
    let mut state = LendingPoolTestState::new();

    // Remove EGLD from e-mode category 1
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

#[test]
fn test_remove_asset_from_e_mode_category_not_found_error() {
    let mut state = LendingPoolTestState::new();

    // Try to remove asset from non-existent category
    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        99,
        ERROR_EMODE_CATEGORY_NOT_FOUND,
    );
}

#[test]
fn test_remove_asset_from_e_mode_category_asset_not_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Try to remove non-existent asset
    let new_token = TestTokenIdentifier::new("NOASSET-123456");
    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(new_token.to_token_identifier()),
        1,
        ERROR_ASSET_NOT_SUPPORTED,
    );
}

#[test]
fn test_remove_asset_from_e_mode_category_asset_not_in_category_error() {
    let mut state = LendingPoolTestState::new();

    // Try to remove USDC which is not in category 1
    state.remove_asset_from_e_mode_category_error(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        1,
        ERROR_ASSET_NOT_SUPPORTED_IN_EMODE,
    );
}

// ============================================
// ASSET CONFIGURATION TESTS
// ============================================

#[test]
fn test_edit_asset_config_success() {
    let mut state = LendingPoolTestState::new();

    // Edit EGLD asset config
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
    let expected = BigUint::from(7000u64);
    assert_eq!(ltv_value, expected);
}

#[test]
fn test_edit_asset_config_asset_not_supported_error() {
    let mut state = LendingPoolTestState::new();

    // Try to edit config for non-existent asset
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

#[test]
fn test_edit_asset_config_invalid_liquidation_threshold_error() {
    let mut state = LendingPoolTestState::new();

    // Try to set liquidation threshold lower than LTV
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(8000u64), // 80% LTV
        &BigUint::from(7000u64), // 70% liquidation threshold (invalid)
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

#[test]
fn test_complete_e_mode_lifecycle() {
    let mut state = LendingPoolTestState::new();

    // 1. Add a new e-mode category
    state.add_e_mode_category(
        BigUint::from(9000u64), // 90% LTV
        BigUint::from(9500u64), // 95% liquidation threshold
        BigUint::from(100u64),  // 1% liquidation bonus
    );

    // 2. Add USDC to the new category
    state.add_asset_to_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
        true,
        true,
    );

    // 3. Edit the asset configuration in the category
    let config = EModeAssetConfig {
        is_collateralizable: true,
        is_borrowable: false,
    };
    state.edit_asset_in_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
        config,
    );

    // 4. Remove the asset from the category
    state.remove_asset_from_e_mode_category(
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        2,
    );

    // 5. Remove the category
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

#[test]
fn test_remove_e_mode_category_with_multiple_assets() {
    let mut state = LendingPoolTestState::new();

    // 1. Add a new e-mode category
    state.add_e_mode_category(
        BigUint::from(8500u64), // 85% LTV
        BigUint::from(9000u64), // 90% liquidation threshold
        BigUint::from(200u64),  // 2% liquidation bonus
    );

    // 2. Add multiple assets to the category
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

    // 3. Remove the category (should remove all assets from it)
    state.remove_e_mode_category(2);

    // Verify category was marked as deprecated
    let e_modes = state.get_e_modes();
    let found = e_modes.into_iter().find(|item| {
        let (id, _) = item.clone().into_tuple();
        id == 2
    });
    assert!(found.is_some());
    let (_, category) = found.unwrap().into_tuple();
    assert!(category.is_deprecated);

    // Verify all assets were removed from the category
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

    // Verify the category's asset list is empty
    let e_mode_assets = state.get_e_modes_assets(2);
    assert_eq!(e_mode_assets.len(), 0);
}
#[test]
fn test_edit_asset_config_with_zero_caps() {
    let mut state = LendingPoolTestState::new();

    // First, set non-zero caps
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64),    // 75% LTV
        &BigUint::from(8000u64),    // 80% liquidation threshold
        &BigUint::from(500u64),     // 5% liquidation bonus
        &BigUint::from(100u64),     // 1% liquidation fees
        false,                      // not isolated
        &BigUint::zero(),           // no debt ceiling
        false,                      // not siloed
        true,                       // flashloanable
        &BigUint::from(10u64),      // 0.1% flash loan fee
        true,                       // collateralizable
        true,                       // borrowable
        false,                      // isolation borrow not enabled
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

    // Now set caps to zero (should result in None)
    state.edit_asset_config(
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        &BigUint::from(7500u64), // 75% LTV
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
        &BigUint::zero(),        // zero borrow cap
        &BigUint::zero(),        // zero supply cap
        None,
    );

    // Verify caps were set to None
    let config_after = state.get_asset_config(EgldOrEsdtTokenIdentifier::esdt(
        EGLD_TOKEN.to_token_identifier(),
    ));
    assert!(config_after.borrow_cap.is_none());
    assert!(config_after.supply_cap.is_none());

    // Verify other values remained the same
    let ltv_value = config_after.loan_to_value.into_raw_units().clone();
    assert_eq!(ltv_value, BigUint::from(7500u64));
}
