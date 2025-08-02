#![no_std]

multiversx_sc::imports!();

#[multiversx_sc::contract]
pub trait SwapMock {
    #[init]
    fn init(&self) {}

    #[upgrade]
    fn upgrade(&self) {}

    // Success case of a flash loan repayment endpoint called by the flash action
    #[payable("*")]
    #[endpoint(swap)]
    fn swap(&self, wanted_token: EgldOrEsdtTokenIdentifier, wanted_amount: BigUint) {
        let caller = self.blockchain().get_caller();

        let payment = EgldOrEsdtTokenPayment::new(wanted_token, 0, wanted_amount);

        // To be refunded to the caller
        self.tx().to(&caller).egld(BigUint::from(10u64)).transfer();

        // To be swapped
        self.tx().to(&caller).payment(payment).transfer();
    }
}
