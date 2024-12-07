#![allow(clippy::too_many_arguments)]

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use common_events::{AssetConfig, EModeAssetConfig, EModeCategory};

use crate::{
    oracle, storage, ERROR_ASSET_ALREADY_SUPPORTED, ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE,
    ERROR_ASSET_NOT_SUPPORTED, ERROR_ASSET_NOT_SUPPORTED_IN_EMODE, ERROR_EMODE_CATEGORY_NOT_FOUND,
    ERROR_INVALID_AGGREGATOR, ERROR_INVALID_LIQUIDATION_THRESHOLD,
    ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE, ERROR_INVALID_TICKER, ERROR_NO_POOL_FOUND,
};

use super::factory;

#[multiversx_sc::module]
pub trait RouterModule:
    factory::FactoryModule
    + common_checks::ChecksModule
    + storage::LendingStorageModule
    + common_events::EventsModule
    + oracle::OracleModule
{
    #[allow_multiple_var_args]
    #[only_owner]
    #[endpoint(createLiquidityPool)]
    fn create_liquidity_pool(
        &self,
        base_asset: EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_bonus: BigUint,
        liquidation_base_fee: BigUint,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        is_isolated: bool,
        debt_ceiling_usd: BigUint,
        flash_loan_fee: BigUint,
        is_siloed: bool,
        flashloan_enabled: bool,
        can_borrow_in_isolation: bool,
        borrow_cap: OptionalValue<BigUint>,
        supply_cap: OptionalValue<BigUint>,
    ) -> ManagedAddress {
        require!(
            self.pools_map(&base_asset).is_empty(),
            ERROR_ASSET_ALREADY_SUPPORTED
        );
        require!(base_asset.is_valid(), ERROR_INVALID_TICKER);

        let address = self.create_pool(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
        );

        self.require_non_zero_address(&address);

        self.pools_map(&base_asset).set(address.clone());
        self.pools_allowed().insert(address.clone());

        let asset_config = &AssetConfig {
            ltv,
            liquidation_threshold,
            liquidation_bonus,
            liquidation_base_fee,
            borrow_cap: borrow_cap.into_option(),
            supply_cap: supply_cap.into_option(),
            can_be_collateral,
            can_be_borrowed,
            is_e_mode_enabled: false,
            is_isolated,
            debt_ceiling_usd,
            flash_loan_fee,
            is_siloed,
            flashloan_enabled,
            can_borrow_in_isolation,
        };

        self.asset_config(&base_asset).set(asset_config);

        self.create_market_params_event(
            &base_asset,
            &r_max,
            &r_base,
            &r_slope1,
            &r_slope2,
            &u_optimal,
            &reserve_factor,
            &address,
            asset_config,
        );
        address
    }

    #[only_owner]
    #[endpoint(upgradeLiquidityPool)]
    fn upgrade_liquidity_pool(
        &self,
        base_asset: &EgldOrEsdtTokenIdentifier,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
    ) {
        require!(!self.pools_map(base_asset).is_empty(), ERROR_NO_POOL_FOUND);

        let pool_address = self.get_pool_address(base_asset);
        self.upgrade_pool(
            pool_address,
            r_max,
            r_base,
            r_slope1,
            r_slope2,
            u_optimal,
            reserve_factor,
        );
    }

    #[only_owner]
    #[endpoint(setAggregator)]
    fn set_aggregator(&self, aggregator: ManagedAddress) {
        require!(!aggregator.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&aggregator),
            ERROR_INVALID_AGGREGATOR
        );
        self.price_aggregator_address().set(&aggregator);
    }

    #[only_owner]
    #[endpoint(setLiquidityPoolTemplate)]
    fn set_liquidity_pool_template(&self, address: ManagedAddress) {
        require!(!address.is_zero(), ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE);

        require!(
            self.blockchain().is_smart_contract(&address),
            ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE
        );
        self.liq_pool_template_address().set(&address);
    }

    #[only_owner]
    #[endpoint(addEModeCategory)]
    fn add_e_mode_category(
        &self,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_bonus: BigUint,
    ) {
        let map = self.last_e_mode_category_id();

        let last_id = map.get();
        let category = EModeCategory {
            id: last_id + 1,
            ltv,
            liquidation_threshold,
            liquidation_bonus,
        };

        map.set(category.id);

        self.update_e_mode_category_event(&category);
        self.e_mode_category().insert(category.id, category);
    }

    #[only_owner]
    #[endpoint(editEModeCategory)]
    fn edit_e_mode_category(&self, category: EModeCategory<Self::Api>) {
        let mut map = self.e_mode_category();
        require!(
            map.contains_key(&category.id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        self.update_e_mode_category_event(&category);
        map.insert(category.id, category);
    }

    #[only_owner]
    #[endpoint(removeEModeCategory)]
    fn remove_e_mode_category(&self, category_id: u8) {
        let mut map = self.e_mode_category();
        require!(
            map.contains_key(&category_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        let assets = self
            .e_mode_assets(category_id)
            .keys()
            .collect::<ManagedVec<_>>();

        for asset in &assets {
            self.remove_asset_from_e_mode_category(asset, category_id);
        }

        let removed_category = map.remove(&category_id);
        self.update_e_mode_category_event(&removed_category.unwrap());
    }

    #[only_owner]
    #[endpoint(addAssetToEModeCategory)]
    fn add_asset_to_e_mode_category(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        category_id: u8,
        can_be_collateral: bool,
        can_be_borrowed: bool,
    ) {
        require!(
            self.e_mode_category().contains_key(&category_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );

        let mut e_mode_assets = self.e_mode_assets(category_id);
        require!(
            !e_mode_assets.contains_key(&asset),
            ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE
        );

        let mut asset_e_modes = self.asset_e_modes(&asset);
        require!(
            !asset_e_modes.contains(&category_id),
            ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE
        );

        let asset_map = self.asset_config(&asset);

        let mut asset_data = asset_map.get();

        if !asset_data.is_e_mode_enabled {
            asset_data.is_e_mode_enabled = true;

            self.update_asset_config_event(&asset, &asset_data);
            asset_map.set(asset_data);
        }
        let e_mode_asset_config = EModeAssetConfig {
            can_be_collateral,
            can_be_borrowed,
        };
        self.update_e_mode_asset_event(&asset, &e_mode_asset_config, category_id);
        asset_e_modes.insert(category_id);
        e_mode_assets.insert(asset, e_mode_asset_config);
    }

    #[only_owner]
    #[endpoint(editAssetInEModeCategory)]
    fn edit_asset_in_e_mode_category(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        category_id: u8,
        config: EModeAssetConfig,
    ) {
        let mut map = self.e_mode_assets(category_id);
        require!(!map.is_empty(), ERROR_EMODE_CATEGORY_NOT_FOUND);
        require!(map.contains_key(&asset), ERROR_ASSET_NOT_SUPPORTED_IN_EMODE);

        self.update_e_mode_asset_event(&asset, &config, category_id);
        map.insert(asset, config);
    }

    #[only_owner]
    #[endpoint(removeAssetFromEModeCategory)]
    fn remove_asset_from_e_mode_category(&self, asset: EgldOrEsdtTokenIdentifier, category_id: u8) {
        let mut e_mode_assets = self.e_mode_assets(category_id);
        require!(!e_mode_assets.is_empty(), ERROR_EMODE_CATEGORY_NOT_FOUND);
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );
        require!(
            e_mode_assets.contains_key(&asset),
            ERROR_ASSET_NOT_SUPPORTED_IN_EMODE
        );

        let config = e_mode_assets.remove(&asset);
        let mut asset_e_modes = self.asset_e_modes(&asset);
        asset_e_modes.swap_remove(&category_id);

        self.update_e_mode_asset_event(&asset, &config.unwrap(), category_id);
        if asset_e_modes.is_empty() {
            let mut asset_data = self.asset_config(&asset).get();
            asset_data.is_e_mode_enabled = false;

            self.update_asset_config_event(&asset, &asset_data);
            self.asset_config(&asset).set(asset_data);
        }
    }
    
    #[allow_multiple_var_args]
    #[only_owner]
    #[endpoint(editAssetConfig)]
    fn edit_asset_config(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_bonus: BigUint,
        liquidation_base_fee: BigUint,
        is_isolated: bool,
        debt_ceiling_usd: BigUint,
        is_siloed: bool,
        flashloan_enabled: bool,
        flash_loan_fee: BigUint,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        can_borrow_in_isolation: bool,
        borrow_cap: OptionalValue<BigUint>,
        supply_cap: OptionalValue<BigUint>,
    ) {
        require!(
            !self.pools_map(&asset).is_empty(),
            ERROR_ASSET_NOT_SUPPORTED
        );

        let map = self.asset_config(&asset);
        require!(!map.is_empty(), ERROR_ASSET_NOT_SUPPORTED);

        require!(
            liquidation_threshold > ltv,
            ERROR_INVALID_LIQUIDATION_THRESHOLD
        );

        let old_config = map.get();

        let new_config = &AssetConfig {
            ltv,
            liquidation_threshold,
            liquidation_bonus,
            liquidation_base_fee,
            is_e_mode_enabled: old_config.is_e_mode_enabled,
            is_isolated,
            debt_ceiling_usd,
            is_siloed,
            flashloan_enabled,
            flash_loan_fee,
            can_be_collateral,
            can_be_borrowed,
            can_borrow_in_isolation,
            borrow_cap: borrow_cap.into_option(),
            supply_cap: supply_cap.into_option(),
        };

        map.set(new_config);

        self.update_asset_config_event(&asset, &new_config);
    }

    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }
}
