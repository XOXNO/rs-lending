#![no_std]

multiversx_sc::imports!();

pub use common_structs::*;

/// The EventsModule trait defines all the events emitted by the Liquidity Pool Smart Contract.
/// These events cover market parameter creation and updates, market state changes, position updates,
/// and configuration changes. All events use indexed parameters for efficient querying.
#[multiversx_sc::module]
pub trait EventsModule {
    /// Emits an event when a new market is created with its initial parameters.
    ///
    /// # Parameters
    /// - `base_asset`: The asset identifier for the market.
    /// - `r_max`: The maximum borrow rate.
    /// - `r_base`: The base borrow rate.
    /// - `r_slope1`: The slope of the rate before reaching optimal utilization.
    /// - `r_slope2`: The slope of the rate after optimal utilization.
    /// - `u_optimal`: The optimal utilization ratio.
    /// - `reserve_factor`: The fraction of accrued interest reserved as protocol fee.
    /// - `market_address`: The address of the deployed market contract.
    /// - `config`: The asset configuration details.
    ///
    /// # Returns
    /// - Nothing.
    #[event("create_market_params")]
    fn create_market_params_event(
        &self,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] r_max: &BigUint,
        #[indexed] r_base: &BigUint,
        #[indexed] r_slope1: &BigUint,
        #[indexed] r_slope2: &BigUint,
        #[indexed] r_slope3: &BigUint,
        #[indexed] u_mid: &BigUint,
        #[indexed] u_optimal: &BigUint,
        #[indexed] reserve_factor: &BigUint,
        #[indexed] market_address: &ManagedAddress,
        #[indexed] config: &AssetConfig<Self::Api>,
    );

    /// Emits an event when market parameters are updated.
    ///
    /// # Parameters
    /// - `base_asset`: The asset identifier for the market.
    /// - `r_max`: The updated maximum borrow rate.
    /// - `r_base`: The updated base rate.
    /// - `r_slope1`: The updated slope before optimal utilization.
    /// - `r_slope2`: The updated slope after optimal utilization.
    /// - `u_optimal`: The updated optimal utilization ratio.
    /// - `reserve_factor`: The updated reserve factor.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_market_params")]
    fn market_params_event(
        &self,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] r_max: &BigUint,
        #[indexed] r_base: &BigUint,
        #[indexed] r_slope1: &BigUint,
        #[indexed] r_slope2: &BigUint,
        #[indexed] r_slope3: &BigUint,
        #[indexed] u_mid: &BigUint,
        #[indexed] u_optimal: &BigUint,
        #[indexed] reserve_factor: &BigUint,
    );

    /// Emits an event to update the overall market state.
    ///
    /// This function is a helper that wraps the lower-level `_emit_update_market_state_event` event,
    /// converting various ManagedDecimal values into raw BigUint values.
    ///
    /// # Parameters
    /// - `timestamp`: The current timestamp.
    /// - `supply_index`: The current supply index.
    /// - `borrow_index`: The current borrow index.
    /// - `reserves`: The current pool reserves.
    /// - `supplied_amount`: The total supplied amount.
    /// - `borrowed_amount`: The total borrowed amount.
    /// - `protocol_revenue`: The accrued protocol revenue.
    /// - `base_asset`: The asset identifier for the market.
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - Nothing.
    fn update_market_state_event(
        &self,
        timestamp: u64,
        supply_index: &ManagedDecimal<Self::Api, NumDecimals>,
        borrow_index: &ManagedDecimal<Self::Api, NumDecimals>,
        reserves: &ManagedDecimal<Self::Api, NumDecimals>,
        supplied_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        protocol_revenue: &ManagedDecimal<Self::Api, NumDecimals>,
        base_asset: &EgldOrEsdtTokenIdentifier,
        asset_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        self._emit_update_market_state_event(
            timestamp,
            supply_index,
            borrow_index,
            reserves,
            supplied_amount,
            borrowed_amount,
            protocol_revenue,
            base_asset,
            asset_price,
        );
    }

    /// Low-level event emitting the updated market state.
    ///
    /// # Parameters
    /// - `timestamp`: The current timestamp.
    /// - `supply_index`: The supply index as a raw BigUint.
    /// - `borrow_index`: The borrow index as a raw BigUint.
    /// - `reserves`: The current reserves as a raw BigUint.
    /// - `supplied_amount`: The total supplied amount as a raw BigUint.
    /// - `borrowed_amount`: The total borrowed amount as a raw BigUint.
    /// - `protocol_revenue`: The protocol revenue as a raw BigUint.
    /// - `base_asset`: The asset identifier for the market.
    /// - `asset_price`: The current asset price.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_market_state")]
    fn _emit_update_market_state_event(
        &self,
        #[indexed] timestamp: u64,
        #[indexed] supply_index: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] borrow_index: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] reserves: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] supplied_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] borrowed_amount: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] protocol_revenue: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] asset_price: &ManagedDecimal<Self::Api, NumDecimals>,
    );

    /// Emits an event to update an account's position.
    ///
    /// This event can be triggered by multiple actions including adding collateral, removing collateral,
    /// borrowing, repaying, accruing interest, or liquidation.
    ///
    /// # Parameters
    /// - `amount`: The amount associated with the position update.
    /// - `position`: The updated account position.
    /// - `asset_price`: An optional asset price used to update market state.
    /// - `caller`: An optional address of the caller. When absent, the update is protocol-driven (e.g., accrued interest).
    /// - `account_attributes`: Optional NFT account attributes for the position.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_position")]
    fn update_position_event(
        &self,
        #[indexed] amount: &ManagedDecimal<Self::Api, NumDecimals>,
        #[indexed] position: &AccountPosition<Self::Api>,
        #[indexed] asset_price: OptionalValue<ManagedDecimal<Self::Api, NumDecimals>>,
        #[indexed] caller: OptionalValue<&ManagedAddress>, // When is none, then the position is updated by the protocol and the amount is the interest, either for borrow or supply
        #[indexed] account_attributes: OptionalValue<&NftAccountAttributes>,
    );

    /// Emits an event to update the debt ceiling for an asset.
    ///
    /// # Parameters
    /// - `asset`: The asset identifier.
    /// - `amount`: The new debt ceiling amount.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_debt_ceiling")]
    fn update_debt_ceiling_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] amount: ManagedDecimal<Self::Api, NumDecimals>,
    );

    /// Emits an event to update the supplied amount in the global vault of an market.
    ///
    /// # Parameters
    /// - `asset`: The asset identifier.
    /// - `amount`: The updated vault supplied amount.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_vault_supplied_amount")]
    fn update_vault_supplied_amount_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] amount: ManagedDecimal<Self::Api, NumDecimals>,
    );

    /// Emits an event when the asset configuration is updated.
    ///
    /// # Parameters
    /// - `asset`: The asset identifier.
    /// - `config`: The new asset configuration.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_asset_config")]
    fn update_asset_config_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] config: &AssetConfig<Self::Api>,
    );

    /// Emits an event when an e-mode category is updated.
    ///
    /// # Parameters
    /// - `category`: The updated e-mode category configuration.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_e_mode_category")]
    fn update_e_mode_category_event(&self, #[indexed] category: &EModeCategory<Self::Api>);

    /// Emits an event when an asset's e-mode configuration is updated.
    ///
    /// # Parameters
    /// - `asset`: The asset identifier.
    /// - `config`: The updated e-mode asset configuration.
    /// - `category_id`: The identifier of the e-mode category.
    ///
    /// # Returns
    /// - Nothing.
    #[event("update_e_mode_asset")]
    fn update_e_mode_asset_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] config: &EModeAssetConfig,
        #[indexed] category_id: u8,
    );
}
