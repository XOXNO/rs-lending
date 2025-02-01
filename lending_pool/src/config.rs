multiversx_sc::imports!();

pub use common_events::*;

use crate::errors::*;
use crate::helpers;
use crate::oracle;
use crate::proxies::*;
use crate::storage;
use crate::utils;

#[multiversx_sc::module]
pub trait ConfigModule:
    storage::LendingStorageModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + oracle::OracleModule
    + helpers::math::MathsModule
{
    #[only_owner]
    #[payable("EGLD")]
    #[endpoint(registerAccountToken)]
    fn register_account_token(&self, token_name: ManagedBuffer, ticker: ManagedBuffer) {
        let payment_amount = self.call_value().egld();
        self.account_token().issue_and_set_all_roles(
            EsdtTokenType::NonFungible,
            payment_amount.clone_value(),
            token_name,
            ticker,
            1,
            None,
        );
    }

    #[only_owner]
    #[endpoint(setTokenOracle)]
    fn set_token_oracle(
        &self,
        market_token: &EgldOrEsdtTokenIdentifier,
        decimals: u8,
        contract_address: &ManagedAddress,
        pricing_method: PricingMethod,
        token_type: OracleType,
        source: ExchangeSource,
        first_tolerance: BigUint,
        last_tolerance: BigUint,
    ) {
        let mapper = self.token_oracle(market_token);

        require!(mapper.is_empty(), ERROR_ORACLE_TOKEN_NOT_FOUND);

        let first_token_id = match source {
            ExchangeSource::LXOXNO => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
                    .main_token()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            }
            ExchangeSource::XExchange => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_xexchange_pair::PairProxy)
                    .first_token_id()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            }
            ExchangeSource::XEGLD => EgldOrEsdtTokenIdentifier::egld(),
            ExchangeSource::LEGLD => EgldOrEsdtTokenIdentifier::egld(),
            _ => {
                panic!("Invalid exchange source")
            }
        };

        let second_token_id = match source {
            ExchangeSource::XExchange => {
                let token_id = self
                    .tx()
                    .to(contract_address)
                    .typed(proxy_xexchange_pair::PairProxy)
                    .second_token_id()
                    .returns(ReturnsResult)
                    .sync_call_readonly();
                EgldOrEsdtTokenIdentifier::esdt(token_id)
            }
            ExchangeSource::XEGLD => first_token_id.clone(),
            ExchangeSource::LEGLD => first_token_id.clone(),
            ExchangeSource::LXOXNO => first_token_id.clone(),
            _ => {
                panic!("Invalid exchange source")
            }
        };

        let tolerance = self.get_anchor_tolerances(&first_tolerance, &last_tolerance);

        let oracle = OracleProvider {
            decimals,
            contract_address: contract_address.clone(),
            pricing_method,
            token_type,
            source,
            first_token_id,
            second_token_id,
            tolerance,
        };

        mapper.set(&oracle);
    }

    #[only_owner]
    #[endpoint(editTokenOracleTolerance)]
    fn edit_token_oracle_tolerance(
        &self,
        market_token: &EgldOrEsdtTokenIdentifier,
        first_tolerance: BigUint,
        last_tolerance: BigUint,
    ) {
        require!(
            !self.token_oracle(market_token).is_empty(),
            ERROR_ORACLE_TOKEN_NOT_FOUND
        );

        let tolerance = self.get_anchor_tolerances(&first_tolerance, &last_tolerance);
        self.token_oracle(market_token).update(|oracle| {
            oracle.tolerance = tolerance;
        });
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
    #[endpoint(setAccumulator)]
    fn set_accumulator(&self, accumulator: ManagedAddress) {
        require!(!accumulator.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&accumulator),
            ERROR_INVALID_AGGREGATOR
        );
        self.accumulator_address().set(&accumulator);
    }

    #[only_owner]
    #[endpoint(setSafePriceView)]
    fn set_safe_price_view(&self, safe_view_address: ManagedAddress) {
        require!(!safe_view_address.is_zero(), ERROR_INVALID_AGGREGATOR);

        require!(
            self.blockchain().is_smart_contract(&safe_view_address),
            ERROR_INVALID_AGGREGATOR
        );
        self.safe_price_view().set(&safe_view_address);
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
            is_deprecated: false,
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
            self.remove_asset_from_e_mode_category(asset.clone_value(), category_id);
        }
        let mut old_info = map.get(&category_id).unwrap();
        old_info.is_deprecated = true;

        self.update_e_mode_category_event(&old_info);

        map.insert(category_id, old_info);
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

    #[only_owner]
    #[endpoint(editAssetConfig)]
    fn edit_asset_config(
        &self,
        asset: EgldOrEsdtTokenIdentifier,
        ltv: BigUint,
        liquidation_threshold: BigUint,
        liquidation_base_bonus: BigUint,
        liquidation_max_fee: BigUint,
        is_isolated: bool,
        debt_ceiling_usd: BigUint,
        is_siloed: bool,
        flashloan_enabled: bool,
        flash_loan_fee: BigUint,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        can_borrow_in_isolation: bool,
        borrow_cap: BigUint,
        supply_cap: BigUint,
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
            liquidation_base_bonus,
            liquidation_max_fee,
            is_e_mode_enabled: old_config.is_e_mode_enabled,
            is_isolated,
            debt_ceiling_usd,
            is_siloed,
            flashloan_enabled,
            flash_loan_fee,
            can_be_collateral,
            can_be_borrowed,
            can_borrow_in_isolation,
            borrow_cap: if borrow_cap == BigUint::zero() {
                None
            } else {
                Some(borrow_cap)
            },
            supply_cap: if supply_cap == BigUint::zero() {
                None
            } else {
                Some(supply_cap)
            },
        };

        map.set(new_config);

        self.update_asset_config_event(&asset, &new_config);
    }
}
