// Debug test for signed number rescaling

use common_math::SharedMathModule;
use multiversx_sc::types::{BigInt, ManagedDecimalSigned};
use multiversx_sc_scenario::api::StaticApi;

pub struct MathTester;
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}
impl SharedMathModule for MathTester {}

#[test]
fn debug_signed_rescale() {
    // Test how -0.5 gets rescaled to 0 decimals
    let neg_half = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(-5i64), // -0.5
        1,
    );

    println!("Original: {} (scale {})", neg_half, neg_half.scale());

    let rescaled = neg_half.rescale(0);
    println!(
        "Rescaled to 0 decimals: {} (raw: {:?})",
        rescaled,
        rescaled.into_raw_units()
    );

    // Test how 1.0 gets rescaled to 0 decimals
    let one = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(10i64), // 1.0
        1,
    );

    let rescaled_one = one.rescale(0);
    println!(
        "1.0 rescaled to 0 decimals: {} (raw: {:?})",
        rescaled_one,
        rescaled_one.into_raw_units()
    );
}
