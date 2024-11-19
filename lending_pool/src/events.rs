use common_structs::{BorrowPosition, DepositPosition};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait EventsModule {
    #[event("add_new_market")]
    fn add_new_market_event(
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

    #[event("new_account")]
    fn new_account_event(&self, #[indexed] account_address: &ManagedAddress, #[indexed] nonce: u64);

    // This can come from two actions:
    // 1. Add collateral -> amount represents the new collateral added
    // 2. Remove collateral -> amount represents the collateral removed
    #[event("update_deposit_position")]
    fn update_deposit_position_event(
        &self,
        #[indexed] nonce: u64,
        #[indexed] amount: &BigUint,
        #[indexed] position: &DepositPosition<Self::Api>,
        #[indexed] caller: &ManagedAddress,
    );

    // This can come from two actions:
    // 1. Borrow -> amount represents the new borrow amount
    // 2. Repay -> amount represents the amount repaid
    #[event("update_borrow_position")]
    fn update_borrow_position_event(
        &self,
        #[indexed] nonce: u64,
        #[indexed] amount: &BigUint,
        #[indexed] position: &BorrowPosition<Self::Api>,
        #[indexed] caller: &ManagedAddress,
    );
}
