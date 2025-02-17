#![no_std]

use common_constants::BPS;

pub mod proxy_lending;
multiversx_sc::imports!();
pub const FLASH_FEES: u128 = 50;

#[multiversx_sc::contract]
pub trait FlashMock {
    #[init]
    fn init(&self) {}

    #[upgrade]
    fn upgrade(&self) {}

    // Success case of a flash loan repayment endpoint called by the flash action
    #[payable("*")]
    #[endpoint(flash)]
    fn flash(&self) {
        let mut payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();

        payment.amount += payment
            .amount
            .clone()
            .mul(BigUint::from(FLASH_FEES))
            .div(BigUint::from(BPS));

        self.tx().to(&caller).payment(payment).transfer();
    }

    // Test a flash loan that repays only a part not all the required fees
    #[payable("*")]
    #[endpoint(flashRepaySome)]
    fn flash_repay_some(&self) {
        let mut payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();

        payment.amount -= payment
            .amount
            .clone()
            .mul(BigUint::from(FLASH_FEES))
            .div(BigUint::from(BPS));

        self.tx().to(&caller).payment(payment).transfer();
    }

    // Fake a scammy flash loan that is not repaying back, tests should fail
    #[payable("*")]
    #[endpoint(flashNoRepay)]
    fn flash_no_repay(&self) {}
}
