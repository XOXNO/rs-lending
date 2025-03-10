use common_structs::{AccountAttributes, AccountPosition, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};

use super::{account, borrow, emode};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionRepayModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + borrow::PositionBorrowModule
    + common_math::SharedMathModule
    + emode::EModeModule
{
    /// Processes a repayment via the liquidity pool.
    /// Updates the borrow position accordingly.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `repay_token_id`: Token being repaid.
    /// - `repay_amount`: Repayment amount.
    /// - `caller`: Repayer's address.
    /// - `borrow_position`: Current borrow position.
    /// - `feed`: Price data for the token.
    /// - `position_attributes`: NFT attributes.
    /// - `cache`: Mutable storage cache.
    fn process_repayment_through_pool(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        mut borrow_position: AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        position_attributes: &AccountAttributes,
        cache: &mut Cache<Self>,
    ) {
        let pool_address = cache.get_cached_pool_address(repay_token_id);
        borrow_position = self
            .tx()
            .to(pool_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(caller, borrow_position.clone(), feed.price.clone())
            .egld_or_single_esdt(repay_token_id, 0, repay_amount.into_raw_units())
            .returns(ReturnsResult)
            .sync_call();

        self.update_position_event(
            repay_amount,
            &borrow_position,
            OptionalValue::Some(feed.price.clone()),
            OptionalValue::Some(caller),
            OptionalValue::Some(position_attributes),
        );

        self.update_borrow_position_storage(account_nonce, repay_token_id, &borrow_position);
    }

    /// Updates isolated debt tracking post-repayment.
    /// Adjusts debt ceiling for isolated positions.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `position`: Borrow position.
    /// - `feed`: Price data for the token.
    /// - `repay_amount`: Repayment amount in EGLD.
    /// - `cache`: Mutable storage cache.
    /// - `position_attributes`: NFT attributes.
    fn update_isolated_debt_after_repayment(
        &self,
        account_nonce: u64,
        position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        repay_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes,
    ) {
        if position_attributes.is_isolated() {
            let collaterals_map = self.deposit_positions(account_nonce);
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();
            let asset_address = self.pools_map(&position.asset_id).get();
            self.update_position(
                &asset_address,
                position,
                OptionalValue::Some(feed.price.clone()),
            );
            let principal_amount = self.calculate_principal_repayment(position, feed, repay_amount);
            let debt_usd_amount =
                self.get_token_usd_value(&principal_amount, &cache.egld_price_feed);
            self.adjust_isolated_debt_usd(&collateral_token_id, debt_usd_amount, false);
        }
    }

    /// Manages the full repayment process.
    /// Validates and updates positions after repayment.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `repay_token_id`: Token being repaid.
    /// - `repay_amount`: Repayment amount.
    /// - `caller`: Repayer's address.
    /// - `repay_amount_in_egld`: EGLD value of repayment.
    /// - `feed`: Price data for the token.
    /// - `cache`: Mutable storage cache.
    /// - `position_attributes`: NFT attributes.
    fn process_repayment(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress,
        repay_amount_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
        feed: &PriceFeedShort<Self::Api>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes,
    ) {
        let mut borrow_position =
            self.validate_borrow_position_existence(account_nonce, repay_token_id);
        self.update_isolated_debt_after_repayment(
            account_nonce,
            &mut borrow_position,
            feed,
            &repay_amount_in_egld,
            cache,
            position_attributes,
        );
        self.process_repayment_through_pool(
            account_nonce,
            repay_token_id,
            repay_amount,
            caller,
            borrow_position,
            feed,
            position_attributes,
            cache,
        );
    }

    /// Ensures a borrow position exists for repayment.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `token_id`: Borrowed token identifier.
    ///
    /// # Returns
    /// - Validated borrow position.
    fn validate_borrow_position_existence(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        let borrow_positions = self.borrow_positions(account_nonce);
        let position = borrow_positions.get(token_id);
        require!(
            position.is_some(),
            "Borrowed token {} is not available for this account",
            token_id
        );
        position.unwrap()
    }

    /// Calculates the principal repaid from a repayment.
    /// Separates principal from interest in repayment.
    ///
    /// # Arguments
    /// - `borrow_position`: Position being repaid.
    /// - `feed`: Price data for the token.
    /// - `amount_to_repay_in_egld`: Repayment amount in EGLD.
    ///
    /// # Returns
    /// - Principal amount repaid in EGLD.
    fn calculate_principal_repayment(
        &self,
        borrow_position: &AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        amount_to_repay_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let interest = self.get_token_egld_value(&borrow_position.interest_accrued, &feed.price);
        let principal = self.get_token_egld_value(&borrow_position.principal_amount, &feed.price);

        if amount_to_repay_in_egld <= &interest {
            return self.wad_zero();
        }

        let diff = amount_to_repay_in_egld.clone() - interest;
        if diff > principal {
            principal
        } else {
            diff
        }
    }

    /// Updates or removes a borrow position in storage.
    /// Reflects repayment changes in storage.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `token_id`: Borrowed token identifier.
    /// - `position`: Updated borrow position.
    fn update_borrow_position_storage(
        &self,
        account_nonce: u64,
        token_id: &EgldOrEsdtTokenIdentifier,
        position: &AccountPosition<Self::Api>,
    ) {
        let mut borrow_positions = self.borrow_positions(account_nonce);
        if position.can_remove() {
            borrow_positions.remove(token_id);
        } else {
            borrow_positions.insert(token_id.clone(), position.clone());
        }
    }
}
