use common_constants::BP;
use common_events::{
    AccountPosition, AccountPositionType, AssetConfig, NftAccountAttributes, PriceFeedShort,
};

use crate::{
    contexts::base::StorageCache, helpers, oracle, proxy_pool, storage, utils, validation,
};

use super::{account, borrow};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionRepayModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + borrow::PositionBorrowModule
{
    /// Processes repayment for a borrow position through liquidity pool
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `caller` - Address initiating repayment
    /// * `borrow_position` - Current borrow position being repaid
    /// * `debt_token_price_data` - Price data for the debt token
    ///
    /// # Returns
    /// * `AccountPosition` - Updated position after repayment
    ///
    /// Calls liquidity pool to process repayment and update interest indices.
    /// If position is fully repaid (amount = 0), removes it from storage.
    /// Otherwise updates storage with new position details.
    fn handle_repay_position(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        caller: &ManagedAddress,
        mut borrow_position: AccountPosition<Self::Api>,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        attributes: &NftAccountAttributes,
    ) {
        let asset_address = self.get_pool_address(repay_token_id);
        borrow_position = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .repay(
                caller,
                borrow_position.clone(),
                &debt_token_price_data.price,
            )
            .egld_or_single_esdt(repay_token_id, 0, repay_amount)
            .returns(ReturnsResult)
            .sync_call();

        // Update storage
        let mut borrow_positions = self.borrow_positions(account_nonce);

        self.update_position_event(
            repay_amount,
            &borrow_position,
            OptionalValue::Some(debt_token_price_data.price.clone()),
            OptionalValue::Some(&caller),
            OptionalValue::Some(attributes),
        );

        if borrow_position.get_total_amount().gt(&BigUint::zero()) {
            borrow_positions.insert(repay_token_id.clone(), borrow_position);
        } else {
            borrow_positions.remove(repay_token_id);
        }
    }

    /// Updates isolated mode debt tracking after repayment
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `principal_usd_amount` - USD value of principal being repaid
    ///
    /// For isolated positions (single collateral), updates the debt ceiling
    /// tracking for the collateral token. This ensures the debt ceiling
    /// is properly decreased when debt is repaid in isolated mode.
    fn handle_isolated_repay(
        &self,
        account_nonce: u64,
        position: &mut AccountPosition<Self::Api>,
        feed: &PriceFeedShort<Self::Api>,
        repay_amount: &BigUint,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) {
        if attributes.is_isolated {
            let collaterals_map = self.deposit_positions(account_nonce);
            let (collateral_token_id, _) = collaterals_map.iter().next().unwrap();

            // 3. Calculate repay amounts
            let asset_address = self.pools_map(&position.token_id).get();

            self.update_position(
                &asset_address,
                position,
                OptionalValue::Some(feed.price.clone()),
            );

            let principal_amount =
                self.validate_and_get_repay_amounts(&position, &feed, repay_amount);

            let debt_usd_amount = self
                .get_token_amount_in_dollars_raw(&principal_amount, &storage_cache.egld_price_feed);

            self.update_isolated_debt_usd(
                &collateral_token_id,
                &debt_usd_amount,
                false, // is_decrease
            );
        }
    }

    /// Processes complete repayment operation
    ///
    /// # Arguments
    /// * `account_nonce` - NFT nonce of the account position
    /// * `repay_token_id` - Token being repaid
    /// * `repay_amount` - Amount being repaid
    /// * `caller` - Address initiating repayment
    /// * `repay_amount_in_egld` - Optional EGLD value of repayment (used in liquidations)
    /// * `debt_token_price_data` - Optional price data (used in liquidations)
    ///
    /// Orchestrates the entire repayment flow:
    /// 1. Validates position exists
    /// 2. Gets or uses provided price data
    /// 3. Calculates repayment amounts
    /// 4. Updates isolated mode debt if applicable
    /// 5. Processes repayment through liquidity pool
    /// 6. Emits position update event
    fn internal_repay(
        &self,
        account_nonce: u64,
        repay_token_id: &EgldOrEsdtTokenIdentifier,
        repay_amount: &BigUint,
        caller: &ManagedAddress,
        repay_amount_in_egld: BigUint,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        storage_cache: &mut StorageCache<Self>,
        attributes: &NftAccountAttributes,
    ) {
        // 1. Validate position exists
        let mut borrow_position = self.validate_borrow_position(account_nonce, repay_token_id);

        // 2. Handle isolated mode debt update
        self.handle_isolated_repay(
            account_nonce,
            &mut borrow_position,
            &debt_token_price_data,
            &repay_amount_in_egld,
            storage_cache,
            attributes,
        );

        // 3. Process repay and update position
        self.handle_repay_position(
            account_nonce,
            repay_token_id,
            repay_amount,
            caller,
            borrow_position,
            &debt_token_price_data,
            attributes,
        );
    }

    /// Validates and calculates repayment amounts
    ///
    /// # Arguments
    /// * `repay_amount` - Amount being repaid
    /// * `borrow_position` - Position being repaid
    /// * `debt_token_price_data` - Price data for debt token
    /// * `repay_amount_in_egld` - Optional EGLD value of repayment
    ///
    /// # Returns
    /// * `BigUint` - EGLD value of principal being repaid
    ///
    fn validate_and_get_repay_amounts(
        &self,
        borrow_position: &AccountPosition<Self::Api>,
        debt_token_price_data: &PriceFeedShort<Self::Api>,
        amount_to_repay_in_egld: &BigUint,
    ) -> BigUint {
        let interest_egld_amount = self.get_token_amount_in_egld_raw(
            &borrow_position.accumulated_interest,
            debt_token_price_data,
        );

        let total_principal_borrowed_egld_amount =
            self.get_token_amount_in_egld_raw(&borrow_position.amount, debt_token_price_data);

        let principal_egld_amount = if amount_to_repay_in_egld > &interest_egld_amount {
            (amount_to_repay_in_egld - &interest_egld_amount)
                .min(total_principal_borrowed_egld_amount)
        } else {
            BigUint::zero()
        };

        principal_egld_amount
    }
}
