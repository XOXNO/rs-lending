use common_structs::{BorrowPosition, DepositPosition};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait EventsModule {
    #[event("upgrade_market")]
    fn upgrade_market_event(
        &self,
        #[indexed] base_asset: &TokenIdentifier,
        #[indexed] r_base: &BigUint,
        #[indexed] r_slope1: &BigUint,
        #[indexed] r_slope2: &BigUint,
        #[indexed] u_optimal: &BigUint,
        #[indexed] reserve_factor: &BigUint,
        #[indexed] liquidation_threshold: &BigUint,
        #[indexed] market_address: &ManagedAddress,
    );

    #[event("add_debt_interest")]
    fn add_debt_interest_event(
        &self,
        #[indexed] nonce: u64,
        #[indexed] amount: &BigUint,
        #[indexed] position: &BorrowPosition<Self::Api>,
    );

    #[event("add_accrued_interest")]
    fn add_accrued_interest_event(
        &self,
        #[indexed] nonce: u64,
        #[indexed] amount: &BigUint,
        #[indexed] position: &DepositPosition<Self::Api>,
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
    );
}
