#![no_std]

multiversx_sc::imports!();

pub use common_structs::*;

#[multiversx_sc::module]
pub trait EventsModule {
    #[event("create_market_params")]
    fn create_market_params_event(
        &self,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] r_max: &BigUint,
        #[indexed] r_base: &BigUint,
        #[indexed] r_slope1: &BigUint,
        #[indexed] r_slope2: &BigUint,
        #[indexed] u_optimal: &BigUint,
        #[indexed] reserve_factor: &BigUint,
        #[indexed] market_address: &ManagedAddress,
        #[indexed] config: &AssetConfig<Self::Api>,
    );

    #[event("update_market_params")]
    fn market_params_event(
        &self,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] r_max: &BigUint,
        #[indexed] r_base: &BigUint,
        #[indexed] r_slope1: &BigUint,
        #[indexed] r_slope2: &BigUint,
        #[indexed] u_optimal: &BigUint,
        #[indexed] reserve_factor: &BigUint,
    );

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
        asset_price: &BigUint,
    ) {
        self._emit_update_market_state_event(
            timestamp,
            supply_index.into_raw_units(),
            borrow_index.into_raw_units(),
            reserves.into_raw_units(),
            supplied_amount.into_raw_units(),
            borrowed_amount.into_raw_units(),
            protocol_revenue.into_raw_units(),
            base_asset,
            asset_price,
        );
    }

    #[event("update_market_state")]
    fn _emit_update_market_state_event(
        &self,
        #[indexed] timestamp: u64,
        #[indexed] supply_index: &BigUint,
        #[indexed] borrow_index: &BigUint,
        #[indexed] reserves: &BigUint,
        #[indexed] supplied_amount: &BigUint,
        #[indexed] borrowed_amount: &BigUint,
        #[indexed] protocol_revenue: &BigUint,
        #[indexed] base_asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] asset_price: &BigUint,
    );

    // This can come from few actions and from both the protocol internal actions and the user actions:
    // 1. Add collateral -> amount represents the new collateral added
    // 2. Remove collateral -> amount represents the collateral removed
    // 3. Borrow -> amount represents the new borrow amount
    // 4. Repay -> amount represents the amount repaid
    // 5. Accrued interest -> amount represents the interest accrued for bororw or supply, based on the position, no caller
    // 6. Liquidation -> amount represents the liquidation amount
    #[event("update_position")]
    fn update_position_event(
        &self,
        #[indexed] amount: &BigUint,
        #[indexed] position: &AccountPosition<Self::Api>,
        #[indexed] asset_price: OptionalValue<BigUint>,
        #[indexed] caller: OptionalValue<ManagedAddress>, // When is none, then the position is updated by the protocol and the amount is the interest, either for borrow or supply
        #[indexed] account_attributes: OptionalValue<NftAccountAttributes>,
    );

    #[event("update_debt_ceiling")]
    fn update_debt_ceiling_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] amount: BigUint,
    );

    #[event("update_vault_supplied_amount")]
    fn update_vault_supplied_amount_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] amount: BigUint,
    );

    #[event("update_asset_config")]
    fn update_asset_config_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] config: &AssetConfig<Self::Api>,
    );

    #[event("update_e_mode_category")]
    fn update_e_mode_category_event(&self, #[indexed] category: &EModeCategory<Self::Api>);

    #[event("update_e_mode_asset")]
    fn update_e_mode_asset_event(
        &self,
        #[indexed] asset: &EgldOrEsdtTokenIdentifier,
        #[indexed] config: &EModeAssetConfig,
        #[indexed] category_id: u8,
    );
}
