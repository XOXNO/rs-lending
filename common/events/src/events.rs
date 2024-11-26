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

    #[event("update_market_state")]
    fn update_market_state_event(
        &self,
        #[indexed] round: u64,
        #[indexed] supply_index: &BigUint,
        #[indexed] borrow_index: &BigUint,
        #[indexed] reserves: &BigUint,
        #[indexed] supplied_amount: &BigUint,
        #[indexed] borrowed_amount: &BigUint,
        #[indexed] rewards_reserve: &BigUint,
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
        #[indexed] caller: Option<&ManagedAddress>, // When is none, then the position is updated by the protocol and the amount is the interest, either for borrow or supply
        #[indexed] account_attributes: Option<NftAccountAttributes>,
    );

    #[event("update_debt_ceiling")]
    fn update_debt_ceiling_event(
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
