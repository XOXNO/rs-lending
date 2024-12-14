multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{
    math, oracle, proxy_pool, storage, ERROR_ASSET_NOT_SUPPORTED, ERROR_BORROW_CAP,
    ERROR_DEBT_CEILING_REACHED, ERROR_MIX_ISOLATED_COLLATERAL, ERROR_NO_POOL_FOUND,
    ERROR_SUPPLY_CAP, ERROR_UNEXPECTED_ANCHOR_TOLERANCES, ERROR_UNEXPECTED_FIRST_TOLERANCE,
    ERROR_UNEXPECTED_LAST_TOLERANCE,
};

use common_structs::*;

pub const EGELD_IDENTIFIER: &str = "EGLD-000000";

pub const MIN_FIRST_TOLERANCE: u128 = 5_000_000_000_000_000_000; // 0.50%
pub const MAX_FIRST_TOLERANCE: u128 = 500_000_000_000_000_000_000; // 50%

pub const MIN_LAST_TOLERANCE: u128 = 12_500_000_000_000_000_000; // 1.5%
pub const MAX_LAST_TOLERANCE: u128 = 1_000_000_000_000_000_000_000; // 100%

#[multiversx_sc::module]
pub trait LendingUtilsModule:
    math::LendingMathModule
    + storage::LendingStorageModule
    + oracle::OracleModule
    + common_events::EventsModule
{
    #[view(getPoolAddress)]
    fn get_pool_address(&self, asset: &EgldOrEsdtTokenIdentifier) -> ManagedAddress {
        let pool_address = self.pools_map(asset).get();

        require!(!pool_address.is_zero(), ERROR_NO_POOL_FOUND);

        pool_address
    }

    #[endpoint(updateIndexes)]
    fn update_indexes(&self, token_id: EgldOrEsdtTokenIdentifier) {
        let pool_address = self.get_pool_address(&token_id);

        let asset_price = self.get_token_price_data(&token_id);
        self.tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .update_indexes(&asset_price.price)
            .sync_call();
    }

    fn get_range(&self, tolerance: &BigUint) -> (BigUint, BigUint) {
        let bp = BigUint::from(BP);
        let upper = &bp + tolerance;
        let lower = &bp * &bp / &upper;

        (upper, lower)
    }

    fn get_anchor_tolerances(
        &self,
        first_tolerance: &BigUint,
        last_tolerance: &BigUint,
    ) -> OraclePriceFluctuation<Self::Api> {
        require!(
            first_tolerance >= &BigUint::from(MIN_FIRST_TOLERANCE)
                && first_tolerance <= &BigUint::from(MAX_FIRST_TOLERANCE),
            ERROR_UNEXPECTED_FIRST_TOLERANCE
        );

        require!(
            last_tolerance >= &BigUint::from(MIN_LAST_TOLERANCE)
                && last_tolerance <= &BigUint::from(MAX_LAST_TOLERANCE),
            ERROR_UNEXPECTED_LAST_TOLERANCE
        );

        require!(
            last_tolerance >= first_tolerance,
            ERROR_UNEXPECTED_ANCHOR_TOLERANCES
        );

        let (first_upper_ratio, first_lower_ratio) = self.get_range(first_tolerance);
        let (last_upper_ratio, last_lower_ratio) = self.get_range(last_tolerance);

        let tolerances = OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        };

        tolerances
    }

    fn get_existing_or_new_deposit_position_for_token(
        &self,
        account_position: u64,
        asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        match self.deposit_positions(account_position).get(token_id) {
            Some(dp) => {
                self.deposit_positions(account_position).remove(token_id);
                dp
            }
            None => AccountPosition::new(
                AccountPositionType::Deposit,
                token_id.clone(),
                BigUint::zero(),
                BigUint::zero(),
                account_position,
                self.blockchain().get_block_timestamp(),
                BigUint::from(BP),
                // Save the current market parameters
                asset_config.liquidation_threshold.clone(),
                is_vault,
            ),
        }
    }

    fn get_existing_or_new_borrow_position_for_token(
        &self,
        account_position: u64,
        asset_config: &AssetConfig<Self::Api>,
        token_id: EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) -> AccountPosition<Self::Api> {
        match self.borrow_positions(account_position).get(&token_id) {
            Some(bp) => bp,
            None => AccountPosition::new(
                AccountPositionType::Borrow,
                token_id,
                BigUint::zero(),
                BigUint::zero(),
                account_position,
                self.blockchain().get_block_timestamp(),
                BigUint::from(BP),
                // Save the current market parameters
                asset_config.liquidation_threshold.clone(),
                is_vault,
            ),
        }
    }

    fn get_liquidation_collateral_in_dollars_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
    ) -> BigUint {
        let mut weighted_collateral_in_dollars = BigUint::zero();

        for dp in positions {
            let position_value_in_dollars =
                self.get_token_amount_in_dollars(&dp.token_id, &dp.get_total_amount());

            weighted_collateral_in_dollars +=
                position_value_in_dollars * &dp.entry_liquidation_threshold / BigUint::from(BP);
        }

        weighted_collateral_in_dollars
    }

    fn get_ltv_collateral_in_dollars_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
    ) -> BigUint {
        let mut weighted_collateral_in_dollars = BigUint::zero();

        for dp in positions {
            let position_value_in_dollars =
                self.get_token_amount_in_dollars(&dp.token_id, &dp.get_total_amount());
            let asset_config = self.asset_config(&dp.token_id).get();

            weighted_collateral_in_dollars +=
                &position_value_in_dollars * &asset_config.ltv / BigUint::from(BP);
        }

        weighted_collateral_in_dollars
    }

    fn get_total_borrow_in_dollars_vec(
        &self,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
    ) -> BigUint {
        let mut total_borrow_in_dollars = BigUint::zero();

        for bp in positions {
            total_borrow_in_dollars +=
                self.get_token_amount_in_dollars(&bp.token_id, &bp.get_total_amount());
        }

        total_borrow_in_dollars
    }

    fn validate_isolated_debt_ceiling(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_to_borrow_in_dollars: &BigUint,
    ) {
        let current_debt = self.isolated_asset_debt_usd(token_id).get();

        let total_debt = current_debt.clone() + amount_to_borrow_in_dollars;

        require!(
            total_debt <= asset_config.debt_ceiling_usd,
            ERROR_DEBT_CEILING_REACHED
        );
    }

    fn update_isolated_debt_usd(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        amount_to_borrow_in_dollars: &BigUint,
        is_increase: bool,
    ) {
        if amount_to_borrow_in_dollars.eq(&BigUint::from(0u64)) {
            return;
        }

        let map = self.isolated_asset_debt_usd(token_id);

        if is_increase {
            map.update(|debt| *debt += amount_to_borrow_in_dollars);
        } else {
            map.update(|debt| *debt -= amount_to_borrow_in_dollars.min(&debt.clone()));
        }

        self.update_debt_ceiling_event(token_id, map.get());
    }

    fn require_asset_supported(&self, asset: &EgldOrEsdtTokenIdentifier) {
        require!(!self.pools_map(asset).is_empty(), ERROR_ASSET_NOT_SUPPORTED);
    }

    fn validate_isolated_collateral(
        &self,
        account_nonce: u64,
        asset_to_deposit: &EgldOrEsdtTokenIdentifier,
    ) {
        let deposit_positions = self.deposit_positions(account_nonce);
        require!(
            deposit_positions.is_empty()
                || (deposit_positions.len() == 1
                    && deposit_positions.contains_key(asset_to_deposit)),
            ERROR_MIX_ISOLATED_COLLATERAL
        );
    }

    fn get_account_attributes(
        &self,
        account_nonce: u64,
        token_id: &TokenIdentifier<Self::Api>,
    ) -> NftAccountAttributes {
        let data = self.blockchain().get_esdt_token_data(
            &self.blockchain().get_sc_address(),
            token_id,
            account_nonce,
        );

        data.decode_attributes::<NftAccountAttributes>()
    }

    fn check_borrow_cap(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        amount: &BigUint,
        asset: &EgldOrEsdtTokenIdentifier,
    ) {
        if asset_config.borrow_cap.is_some() {
            let pool = self.pools_map(asset).get();
            let borrow_cap = asset_config.borrow_cap.clone().unwrap();
            let total_borrow = self
                .tx()
                .to(pool)
                .typed(proxy_pool::LiquidityPoolProxy)
                .borrowed_amount()
                .returns(ReturnsResult)
                .sync_call();

            require!(total_borrow + amount <= borrow_cap, ERROR_BORROW_CAP);
        }
    }

    fn check_supply_cap(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        amount: &BigUint,
        asset: &EgldOrEsdtTokenIdentifier,
        is_vault: bool,
    ) {
        if asset_config.supply_cap.is_some() {
            let pool = self.pools_map(asset).get();
            let supply_cap = asset_config.supply_cap.clone().unwrap();
            let mut total_supply = self
                .tx()
                .to(pool)
                .typed(proxy_pool::LiquidityPoolProxy)
                .supplied_amount()
                .returns(ReturnsResult)
                .sync_call();

            if is_vault {
                total_supply += self.vault_supplied_amount(asset).get();
            }

            require!(total_supply + amount <= supply_cap, ERROR_SUPPLY_CAP);
        }
    }

    fn get_multi_payments(&self) -> ManagedVec<EgldOrEsdtTokenPaymentNew<Self::Api>> {
        let payments = self.call_value().all_esdt_transfers();

        let mut valid_payments = ManagedVec::new();
        for i in 0..payments.len() {
            let payment = payments.get(i);
            // EGLD sent as multi-esdt payment
            if payment.token_identifier.clone().into_managed_buffer()
                == ManagedBuffer::from(EGELD_IDENTIFIER)
                || payment.token_identifier.clone().into_managed_buffer()
                    == ManagedBuffer::from("EGLD")
            {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::egld(),
                    token_nonce: 0,
                    amount: payment.amount.clone(),
                });
            } else {
                valid_payments.push(EgldOrEsdtTokenPaymentNew {
                    token_identifier: EgldOrEsdtTokenIdentifier::esdt(
                        payment.token_identifier.clone(),
                    ),
                    token_nonce: payment.token_nonce,
                    amount: payment.amount.clone(),
                });
            }
        }

        valid_payments
    }
}
