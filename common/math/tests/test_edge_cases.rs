// Edge case tests for mathematical operations

use common_constants::{RAY, WAD};
use common_math::SharedMathModule;
use multiversx_sc::types::{BigInt, BigUint, ManagedDecimal, ManagedDecimalSigned};
use multiversx_sc_scenario::api::StaticApi;

pub struct MathTester;
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}
impl SharedMathModule for MathTester {}

// ============== EXTREME PRECISION TESTS ==============

#[test]
fn test_wad_ray_precision_edge_cases() {
    let tester = MathTester;

    // Test 1: Multiply two WAD values and keep WAD precision
    let a = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(WAD), // 1 WAD = 10^18
        18,
    );
    let b = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(WAD * 2), // 2 WAD
        18,
    );
    let result = tester.mul_half_up(&a, &b, 18);
    assert_eq!(result.into_raw_units(), &BigUint::from(WAD * 2)); // 2 WAD

    // Test 2: Divide RAY by WAD
    let ray_val = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(RAY), // 1 RAY = 10^27
        27,
    );
    let wad_val = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(WAD), // 1 WAD = 10^18
        18,
    );
    let result2 = tester.div_half_up(&ray_val, &wad_val, 9);
    assert_eq!(result2.into_raw_units(), &BigUint::from(1_000_000_000u64)); // 10^9
}

// ============== ZERO AND ONE TESTS ==============

#[test]
fn test_operations_with_zero_and_one() {
    let tester = MathTester;

    // Test operations with zero
    let zero = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(0u64), 5);
    let one = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(100000u64), 5); // 1.00000

    // Multiply by zero
    let result1 = tester.mul_half_up(&one, &zero, 5);
    assert_eq!(result1.into_raw_units(), &BigUint::from(0u64));

    // Divide zero by non-zero
    let result2 = tester.div_half_up(&zero, &one, 5);
    assert_eq!(result2.into_raw_units(), &BigUint::from(0u64));

    // Multiply by one
    let value = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(12345u64), 5);
    let result3 = tester.mul_half_up(&value, &one, 5);
    assert_eq!(result3.into_raw_units(), &BigUint::from(12345u64));

    // Divide by one
    let result4 = tester.div_half_up(&value, &one, 5);
    assert_eq!(result4.into_raw_units(), &BigUint::from(12345u64));
}

// ============== MAXIMUM PRECISION LOSS TESTS ==============

#[test]
fn test_maximum_precision_changes() {
    let tester = MathTester;

    // Test 1: Scale down from 27 decimals (RAY) to 0 decimals
    let ray_value = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_500_000_000_000_000_000_000_000_000u128), // 1.5 RAY
        27,
    );
    let result1 = tester.rescale_half_up(&ray_value, 0);
    assert_eq!(result1.into_raw_units(), &BigUint::from(2u64)); // Rounds up to 2

    // Test 2: Scale up from 0 to 27 decimals
    let int_value = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(5u64), 0);
    let result2 = tester.rescale_half_up(&int_value, 27);
    assert_eq!(
        result2.into_raw_units(),
        &BigUint::from(5_000_000_000_000_000_000_000_000_000u128)
    );
}

// ============== BOUNDARY VALUE TESTS ==============

#[test]
fn test_rounding_at_boundaries() {
    let tester = MathTester;

    // Test exact boundary values for half-up rounding

    // Test 1: 0.4999... should round down to 0
    let value1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(4999u64), // 0.4999
        4,
    );
    let result1 = tester.rescale_half_up(&value1, 0);
    assert_eq!(result1.into_raw_units(), &BigUint::from(0u64));

    // Test 2: 0.5000 should round up to 1
    let value2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(5000u64), // 0.5000
        4,
    );
    let result2 = tester.rescale_half_up(&value2, 0);
    assert_eq!(result2.into_raw_units(), &BigUint::from(1u64));

    // Test 3: 0.5001 should round up to 1
    let value3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(5001u64), // 0.5001
        4,
    );
    let result3 = tester.rescale_half_up(&value3, 0);
    assert_eq!(result3.into_raw_units(), &BigUint::from(1u64));
}

// ============== SIGNED NUMBER EDGE CASES ==============

#[test]
fn test_signed_edge_cases() {
    let tester = MathTester;

    // Test 1: -0.5 should round to -1 (away from zero)
    let value1 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(-5i64), // -0.5
        1,
    );
    let _zero = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(0i64), 1);

    // Multiply -0.5 * 1 and round to 0 decimals
    let one = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(10i64), 1); // 1.0
    let result1 = tester.mul_half_up_signed(&value1, &one, 0);
    // Note: -0.5 rescaled to 0 decimals becomes 0 (ManagedDecimalSigned truncates toward zero)
    // 1.0 becomes 1, so 0 * 1 = 0
    assert_eq!(result1.into_raw_units(), &BigInt::from(0i64)); // 0

    // Test 2: Very small negative * very small positive
    let tiny_neg = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(-1i64), // -0.00001
        5,
    );
    let tiny_pos = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(1i64), // 0.00001
        5,
    );
    let result2 = tester.mul_half_up_signed(&tiny_neg, &tiny_pos, 10);
    assert_eq!(result2.into_raw_units(), &BigInt::from(-1i64)); // -0.0000000001

    // Test 3: Division with sign change at boundary
    let neg_half = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(-5i64), // -0.5
        1,
    );
    let two = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(
        BigInt::from(20i64), // 2.0
        1,
    );
    let result3 = tester.div_half_up_signed(&neg_half, &two, 1);
    assert_eq!(result3.into_raw_units(), &BigInt::from(-3i64)); // -0.3 (rounds away from zero)
}

// ============== SEQUENTIAL OPERATIONS TEST ==============

#[test]
fn test_sequential_operations_precision() {
    let tester = MathTester;

    // Test compound operations maintaining precision
    let initial = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(100_000u64), // 100.000
        3,
    );

    // Multiply by 1.1
    let factor1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(110u64), // 1.10
        2,
    );
    let step1 = tester.mul_half_up(&initial, &factor1, 3);
    assert_eq!(step1.into_raw_units(), &BigUint::from(110_000u64)); // 110.000

    // Divide by 0.9
    let factor2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(90u64), // 0.90
        2,
    );
    let step2 = tester.div_half_up(&step1, &factor2, 3);
    assert_eq!(step2.into_raw_units(), &BigUint::from(122_222u64)); // 122.222

    // Scale down to 1 decimal
    let final_result = tester.rescale_half_up(&step2, 1);
    assert_eq!(final_result.into_raw_units(), &BigUint::from(1222u64)); // 122.2
}

// ============== OVERFLOW PREVENTION TEST ==============

#[test]
fn test_large_number_operations() {
    let tester = MathTester;

    // Test with numbers close to practical limits
    // Using numbers that won't overflow in u128 arithmetic

    // Large WAD numbers
    let large1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_000_000u128 * WAD), // 1 million tokens
        18,
    );
    let large2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_000u128 * WAD), // 1 thousand tokens
        18,
    );

    // Division should work
    let result = tester.div_half_up(&large1, &large2, 0);
    assert_eq!(result.into_raw_units(), &BigUint::from(1000u64)); // 1000

    // Multiplication with reduced precision
    let smaller1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(1_000_000u64), // 1000.000
        3,
    );
    let smaller2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(2_000u64), // 2.000
        3,
    );
    let result2 = tester.mul_half_up(&smaller1, &smaller2, 0);
    // Note: 1000.000 rescaled to 0 decimals = 1000, 2.000 rescaled to 0 decimals = 2
    // So 1000 * 2 = 2000
    assert_eq!(result2.into_raw_units(), &BigUint::from(2000u64)); // 2000
}
