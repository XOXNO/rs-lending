use common_structs::{AccountAttributes, AccountPosition, AccountPositionType, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};

use super::{account, borrow, emode, update};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionRepayModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + borrow::PositionBorrowModule
    + update::PositionUpdateModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
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
        position_attributes: &AccountAttributes<Self::Api>,
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

        self.emit_position_update_event(
            repay_amount,
            &borrow_position,
            feed.price.clone(),
            caller,
            position_attributes,
        );

        self.update_or_remove_position(account_nonce, &borrow_position);
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
        repay_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        if position_attributes.is_isolated() {
            let debt_usd_amount = self.get_egld_usd_value(repay_amount, &cache.egld_usd_price);
            self.adjust_isolated_debt_usd(
                &position_attributes.get_isolated_token(),
                debt_usd_amount,
                false,
            );
        }
    }

    fn clear_position_isolated_debt(
        &self,
        position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        position_attributes: &AccountAttributes<Self::Api>,
        cache: &mut Cache<Self>,
    ) {
        if position_attributes.is_isolated() {
            let amount = self.get_total_amount(position, feed, cache);
            let egld_amount = self.get_token_egld_value(&amount, &feed.price);
            let debt_usd_amount = self.get_egld_usd_value(&egld_amount, &cache.egld_usd_price);
            self.adjust_isolated_debt_usd(
                &position_attributes.get_isolated_token(),
                debt_usd_amount,
                false,
            );
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
        position_attributes: &AccountAttributes<Self::Api>,
    ) {
        let borrow_position =
            self.validate_borrow_position_existence(account_nonce, repay_token_id);

        self.update_isolated_debt_after_repayment(
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
        let borrow_positions = self.positions(account_nonce, AccountPositionType::Borrow);
        let position = borrow_positions.get(token_id);
        require!(
            position.is_some(),
            "Borrowed token {} is not available for this account",
            token_id
        );
        position.unwrap()
    }
}
