// Simple standalone tests for individual math functions
// Run with: cargo test --test test_individual_functions test_name

use multiversx_sc::types::{BigUint, ManagedDecimal};
use multiversx_sc_scenario::api::StaticApi;

// Import the math module
use common_math::SharedMathModule;

// Test struct that implements the trait
pub struct MathTester;

// We need to provide a minimal ContractBase implementation
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}

// Now we can implement SharedMathModule
impl SharedMathModule for MathTester {}

#[test]
fn test_to_decimal_wad() {
    let tester = MathTester;

    // Test converting raw value to WAD decimal (18 decimals)
    let value = BigUint::<StaticApi>::from(1_000_000_000_000_000_000u64); // 1 WAD
    let result = tester.to_decimal_wad(value.clone());

    assert_eq!(result.into_raw_units(), &value);
    assert_eq!(result.scale(), 18);
}

#[test]
fn test_wad_zero() {
    let tester = MathTester;

    let result = tester.wad_zero();
    assert_eq!(result.into_raw_units(), &BigUint::<StaticApi>::zero());
    assert_eq!(result.scale(), 18);
}

#[test]
fn test_ray_zero() {
    let tester = MathTester;

    let result = tester.ray_zero();
    assert_eq!(result.into_raw_units(), &BigUint::<StaticApi>::zero());
    assert_eq!(result.scale(), 27);
}

#[test]
fn test_bps_zero() {
    let tester = MathTester;

    let result = tester.bps_zero();
    assert_eq!(result.into_raw_units(), &BigUint::<StaticApi>::zero());
    assert_eq!(result.scale(), 4);
}

#[test]
fn test_mul_half_up() {
    let tester = MathTester;

    // Test 1.5 * 2.0 = 3.0 with WAD precision
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_500_000_000_000_000_000u64),
        18,
    );
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(2_000_000_000_000_000_000u64),
        18,
    );

    let result = tester.mul_half_up(&a, &b, 18);

    // Should be 3.0 WAD
    assert_eq!(
        result.into_raw_units(),
        &BigUint::<StaticApi>::from(3_000_000_000_000_000_000u64)
    );
}

#[test]
fn test_mul_half_up_rounding() {
    let tester = MathTester;

    // Test rounding: 1.5 * 1.3 = 1.95, which should round to 2.0
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(15u64), 1); // 1.5
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(13u64), 1); // 1.3

    let result = tester.mul_half_up(&a, &b, 1);

    // 1.5 * 1.3 = 1.95, rounds to 2.0
    assert_eq!(result.into_raw_units(), &BigUint::<StaticApi>::from(20u64));
}

#[test]
fn test_div_half_up() {
    let tester = MathTester;

    // Test 3.0 / 2.0 = 1.5 with WAD precision
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(3_000_000_000_000_000_000u64),
        18,
    );
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(2_000_000_000_000_000_000u64),
        18,
    );

    let result = tester.div_half_up(&a, &b, 18);

    // Should be 1.5 WAD
    assert_eq!(
        result.into_raw_units(),
        &BigUint::<StaticApi>::from(1_500_000_000_000_000_000u64)
    );
}

#[test]
fn test_div_half_up_rounding() {
    let tester = MathTester;

    // Test rounding: 5 / 3 = 1.666..., which should round to 1.7 with 1 decimal
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(50u64), 1); // 5.0
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(30u64), 1); // 3.0

    let result = tester.div_half_up(&a, &b, 1);

    // 5.0 / 3.0 = 1.666..., rounds to 1.7
    assert_eq!(result.into_raw_units(), &BigUint::<StaticApi>::from(17u64));
}

#[test]
fn test_rescale_half_up() {
    let tester = MathTester;

    // Test scaling down from 18 to 4 decimals with rounding
    let value = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_234_567_890_123_456_789u64),
        18,
    );
    let result = tester.rescale_half_up(&value, 4);
    let expected = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12346u64), // 1.2346 with 4 decimals
        4,
    );
    // Should round to 1.2346 (4 decimals)
    assert_eq!(result.scale(), 4);
    assert_eq!(result, expected);
}

#[test]
fn test_get_min() {
    let tester = MathTester;

    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(100u64), 2);
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(200u64), 2);

    let result = tester.min(a.clone(), b.clone());
    assert_eq!(result.into_raw_units(), a.into_raw_units());
}

#[test]
fn test_get_max() {
    let tester = MathTester;

    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(100u64), 2);
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(200u64), 2);

    let result = tester.max(a.clone(), b.clone());
    assert_eq!(result.into_raw_units(), b.into_raw_units());
}
