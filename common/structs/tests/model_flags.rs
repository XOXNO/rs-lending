use common_structs::{AssetConfig, EModeAssetConfig};
use multiversx_sc::types::{BigUint, ManagedDecimal, NumDecimals};
use multiversx_sc_scenario::api::StaticApi;

#[test]
fn asset_config_flags_behavior() {
    let zero = ManagedDecimal::<StaticApi, NumDecimals>::from_raw_units(BigUint::zero(), 18);

    let cfg: AssetConfig<StaticApi> = AssetConfig {
        loan_to_value_bps: zero.clone(),
        liquidation_threshold_bps: zero.clone(),
        liquidation_bonus_bps: zero.clone(),
        liquidation_fees_bps: zero.clone(),
        is_collateralizable: true,
        is_borrowable: false,
        e_mode_enabled: true,
        is_isolated_asset: false,
        isolation_debt_ceiling_usd_wad: zero.clone(),
        is_siloed_borrowing: true,
        is_flashloanable: true,
        flashloan_fee_bps: zero.clone(),
        isolation_borrow_enabled: true,
        borrow_cap_wad: None,
        supply_cap_wad: None,
    };

    assert!(cfg.can_supply());
    assert!(!cfg.can_borrow());
    assert!(cfg.has_emode());
    assert!(!cfg.is_isolated());
    assert!(cfg.is_siloed_borrowing());
    assert!(cfg.can_flashloan());
    assert!(cfg.can_borrow_in_isolation());
    let _fee = cfg.flash_loan_fee();
}

#[test]
fn emode_asset_config_flags() {
    let emode = EModeAssetConfig {
        is_collateralizable: true,
        is_borrowable: true,
    };
    assert!(emode.can_borrow());
    // No explicit can_supply on EModeAssetConfig; collateralizable flag used in e-mode application.
    assert!(emode.is_collateralizable);
}
