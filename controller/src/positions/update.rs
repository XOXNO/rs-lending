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
    fn get_borrow_positions(
        &self,
        account_nonce: u64,
        should_return_map: bool,
    ) -> (
        ManagedVec<AccountPosition<Self::Api>>,
        ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) {
        let borrow_positions_map = self.positions(account_nonce, AccountPositionType::Borrow);
        let mut updated_positions = ManagedVec::new();
        let mut position_index_map = ManagedMapEncoded::new();

        for (index, asset_id) in borrow_positions_map.keys().enumerate() {
            if should_return_map {
                let safe_index = index + 1; // Avoid zero index issues
                position_index_map.put(&asset_id, &safe_index);
            }

            updated_positions.push(borrow_positions_map.get(&asset_id).unwrap());
        }

        (updated_positions, position_index_map)
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
        self.positions(account_nonce, position.position_type.clone())
            .insert(position.asset_id.clone(), position.clone());
    }

    /// Stores an updated position in storage.
    /// Handles deposit or borrow position types.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `position`: Updated position.
    fn remove_position(&self, account_nonce: u64, position: &AccountPosition<Self::Api>) {
        self.positions(account_nonce, position.position_type.clone())
            .remove(&position.asset_id);
    }

    /// Updates or removes a borrow position in storage.
    /// Reflects repayment changes in storage.
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce.
    /// - `position`: Updated borrow position.
    fn update_or_remove_position(&self, account_nonce: u64, position: &AccountPosition<Self::Api>) {
        if position.can_remove() {
            self.remove_position(account_nonce, position);
        } else {
            self.store_updated_position(account_nonce, position);
        }
    }

    /// Emits an event for a position update.
    /// Logs interest accruals or changes.
    ///
    /// # Arguments
    /// - `position`: Updated position.
    fn emit_position_update_event(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        position: &AccountPosition<Self::Api>,
        price: ManagedDecimal<Self::Api, NumDecimals>,
        caller: &ManagedAddress<Self::Api>,
        attributes: &AccountAttributes<Self::Api>,
    ) {
        self.update_position_event(
            amount,
            position,
            OptionalValue::Some(price),
            OptionalValue::Some(caller),
            OptionalValue::Some(attributes),
        );
    }
}
