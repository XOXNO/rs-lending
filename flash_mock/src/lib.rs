#![no_std]

use common_constants::BP;
pub mod proxy_lending;
multiversx_sc::imports!();
pub const FLASH_FEES: u128 = 5_000_000_000_000_000_000;

#[multiversx_sc::contract]
pub trait FlashMock {
    #[init]
    fn init(&self) {}

    #[upgrade]
    fn upgrade(&self) {}

    #[payable("*")]
    #[endpoint(flash)]
    fn flash(&self) {
        let mut payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();

        payment.amount += payment
            .amount
            .clone()
            .mul(BigUint::from(FLASH_FEES))
            .div(BigUint::from(BP));

        self.tx().to(&caller).payment(payment).transfer();
    }

    #[payable("*")]
    #[endpoint(flashRepaySome)]
    fn flash_repay_some(&self) {
        let mut payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();

        payment.amount -= payment
            .amount
            .clone()
            .mul(BigUint::from(FLASH_FEES))
            .div(BigUint::from(BP));

        self.tx().to(&caller).payment(payment).transfer();
    }

    #[payable("*")]
    #[endpoint(flashNoRepay)]
    fn flash_no_repay(&self) {}
}
