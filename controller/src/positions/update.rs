use common_structs::{AccountAttributes, AccountPosition, AccountPositionType};

use crate::{cache::Cache, helpers, oracle, storage, utils, validation};

use super::account;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionUpdateModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
    + common_math::SharedMathModule
{
    /// Syncs borrow positions with the liquidity layer.
    /// Updates accrued interest for all borrow positions.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `cache`: Mutable storage cache.
    /// - `should_fetch_price`: Flag to fetch prices.
    /// - `should_return_map`: Flag to return index map.
    ///
    /// # Returns
    /// - Tuple of (updated positions, optional index map).
    fn sync_borrow_positions_interest(
        &self,
        account_nonce: u64,
        cache: &mut Cache<Self>,
        should_fetch_price: bool,
        should_return_map: bool,
    ) -> (
        ManagedVec<AccountPosition<Self::Api>>,
        ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) {
        let borrow_positions_map = self.borrow_positions(account_nonce);
        let mut updated_positions = ManagedVec::new();
        let mut position_index_map = ManagedMapEncoded::new();

        for (index, token_id) in borrow_positions_map.keys().enumerate() {
            let mut borrow_position = borrow_positions_map.get(&token_id).unwrap();
            let pool_address = cache.get_cached_pool_address(&borrow_position.asset_id);
            let price =
                self.fetch_price_if_needed(&borrow_position.asset_id, cache, should_fetch_price);

            self.update_position(&pool_address, &mut borrow_position, price);

            if should_fetch_price {
                self.emit_position_update_event(&borrow_position);
            }

            if should_return_map {
                let safe_index = index + 1; // Avoid zero index issues
                position_index_map.put(&borrow_position.asset_id, &safe_index);
            }

            self.store_updated_position(account_nonce, &borrow_position);
            updated_positions.push(borrow_position.clone());
        }

        (updated_positions, position_index_map)
    }

    /// Syncs deposit positions with the liquidity layer.
    /// Updates accrued interest for non-vault deposits.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `cache`: Mutable storage cache.
    /// - `should_fetch_price`: Flag to fetch prices.
    ///
    /// # Returns
    /// - Vector of updated deposit positions.
    fn sync_deposit_positions_interest(
        &self,
        account_nonce: u64,
        cache: &mut Cache<Self>,
        should_fetch_price: bool,
        account_attributes: &AccountAttributes<Self::Api>,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let deposit_positions_map = self.deposit_positions(account_nonce);
        let mut updated_positions = ManagedVec::new();

        for mut deposit_position in deposit_positions_map.values() {
            if !account_attributes.is_vault() {
                let pool_address = cache.get_cached_pool_address(&deposit_position.asset_id);

                let price = self.fetch_price_if_needed(
                    &deposit_position.asset_id,
                    cache,
                    should_fetch_price,
                );

                self.update_position(&pool_address, &mut deposit_position, price);

                if should_fetch_price {
                    self.emit_position_update_event(&deposit_position);
                }

                self.store_updated_position(account_nonce, &deposit_position);
            }

            updated_positions.push(deposit_position.clone());
        }

        updated_positions
    }

    /// Fetches token price if requested.
    /// Supports conditional price updates.
    ///
    /// # Arguments
    /// - `token_id`: Token identifier.
    /// - `cache`: Mutable storage cache.
    /// - `should_fetch`: Fetch price flag.
    ///
    /// # Returns
    /// - Optional price value.
    fn fetch_price_if_needed(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        cache: &mut Cache<Self>,
        should_fetch: bool,
    ) -> OptionalValue<ManagedDecimal<Self::Api, NumDecimals>> {
        if should_fetch {
            let result = self.get_token_price(token_id, cache);
            OptionalValue::Some(result.price)
        } else {
            OptionalValue::None
        }
    }

    /// Stores an updated position in storage.
    /// Handles deposit or borrow position types.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `position`: Updated position.
    fn store_updated_position(&self, account_nonce: u64, position: &AccountPosition<Self::Api>) {
        match position.position_type {
            common_events::AccountPositionType::Deposit => {
                self.deposit_positions(account_nonce)
                    .insert(position.asset_id.clone(), position.clone());
            },
            AccountPositionType::Borrow => {
                self.borrow_positions(account_nonce)
                    .insert(position.asset_id.clone(), position.clone());
            },
            AccountPositionType::None => {
                panic!("Position type is None");
            },
        }
    }

    /// Emits an event for a position update.
    /// Logs interest accruals or changes.
    ///
    /// # Arguments
    /// - `position`: Updated position.
    fn emit_position_update_event(&self, position: &AccountPosition<Self::Api>) {
        self.update_position_event(
            &position.zero_decimal(),
            position,
            OptionalValue::None,
            OptionalValue::None,
            OptionalValue::None,
        );
    }
}
