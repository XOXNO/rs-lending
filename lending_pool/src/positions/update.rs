use common_events::AccountPosition;

use crate::{contexts::base::StorageCache, helpers, oracle, storage, utils, validation};

use super::account;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionUpdateModule:
    storage::LendingStorageModule
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::math::MathsModule
    + account::PositionAccountModule
{
    /// Updates all borrow positions for an account with accumulated interest
    ///
    /// # Arguments
    /// * `account_position` - The NFT nonce representing the account position
    /// * `fetch_price` - Whether to fetch current price data for each asset
    ///
    /// # Returns
    /// * `ManagedVec<AccountPosition>` - Vector of updated borrow positions
    ///
    /// Updates each borrow position by calling the liquidity pool to calculate
    /// accumulated interest. Stores the updated positions in storage and returns them.
    fn update_debt(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
        return_map: bool,
    ) -> (
        ManagedVec<AccountPosition<Self::Api>>,
        ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) {
        let borrow_positions = self.borrow_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        let mut index_position =
            ManagedMapEncoded::<Self::Api, EgldOrEsdtTokenIdentifier, usize>::new();
        for (index, token_id) in borrow_positions.keys().enumerate() {
            let mut bp = borrow_positions.get(&token_id).unwrap();
            let asset_address = self.get_pool_address(&bp.token_id);
            let price = if fetch_price {
                let result = self.get_token_price(&bp.token_id, storage_cache);
                OptionalValue::Some(result.price)
            } else {
                OptionalValue::None
            };

            self.update_position(&asset_address, &mut bp, price);

            if fetch_price {
                self.update_position_event(
                    &BigUint::zero(),
                    &bp,
                    OptionalValue::None,
                    OptionalValue::None,
                    OptionalValue::None,
                );
            }

            if return_map {
                let safe_index = index + 1;
                index_position.put(&bp.token_id, &safe_index);
            }

            self.borrow_positions(account_position)
                .insert(bp.token_id.clone(), bp.clone());

            positions.push(bp.clone());
        }
        (positions, index_position)
    }



    /// Updates all collateral positions for an account with accumulated interest
    ///
    /// # Arguments
    /// * `account_position` - The NFT nonce representing the account position
    /// * `fetch_price` - Whether to fetch current price data for each asset
    ///
    /// # Returns
    /// * `ManagedVec<AccountPosition>` - Vector of updated collateral positions
    ///
    /// Updates each collateral position by calling the liquidity pool to calculate
    /// accumulated interest. Skips vault positions as they don't accrue interest.
    /// Stores the updated positions in storage and returns them.
    fn update_interest(
        &self,
        account_position: u64,
        storage_cache: &mut StorageCache<Self>,
        fetch_price: bool,
    ) -> ManagedVec<AccountPosition<Self::Api>> {
        let positions_map = self.deposit_positions(account_position);
        let mut positions: ManagedVec<Self::Api, AccountPosition<Self::Api>> = ManagedVec::new();
        for mut dp in positions_map.values() {
            let asset_address = self.get_pool_address(&dp.token_id);
            if !dp.is_vault {
                let price = if fetch_price {
                    let result = self.get_token_price(&dp.token_id, storage_cache);
                    OptionalValue::Some(result.price)
                } else {
                    OptionalValue::None
                };
                self.update_position(&asset_address, &mut dp, price);

                if fetch_price {
                    self.update_position_event(
                        &BigUint::zero(),
                        &dp,
                        OptionalValue::None,
                        OptionalValue::None,
                        OptionalValue::None,
                    );
                }
                self.deposit_positions(account_position)
                    .insert(dp.token_id.clone(), dp.clone());

                positions.push(dp);
            } else {
                positions.push(dp.clone());
            }
        }
        positions
    }

}
