// Comprehensive tests for all rounding functions with edge cases

use multiversx_sc::types::{BigInt, BigUint, ManagedDecimal, ManagedDecimalSigned};
use multiversx_sc_scenario::api::StaticApi;
use common_math::SharedMathModule;

pub struct MathTester;
impl multiversx_sc::contract_base::ContractBase for MathTester {
    type Api = StaticApi;
}
impl SharedMathModule for MathTester {}

// ============== RESCALE_HALF_UP TESTS ==============

#[test]
fn test_rescale_half_up_comprehensive() {
    let tester = MathTester;
    
    // Test 1: Scale down with exact half - should round up
    let value1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12345u64), // 1.2345 with 4 decimals
        4
    );
    let result1 = tester.rescale_half_up(&value1, 3);
    assert_eq!(result1.into_raw_units(), &BigUint::from(1235u64)); // 1.235
    
    // Test 2: Scale down with less than half - should round down
    let value2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12344u64), // 1.2344 with 4 decimals
        4
    );
    let result2 = tester.rescale_half_up(&value2, 3);
    assert_eq!(result2.into_raw_units(), &BigUint::from(1234u64)); // 1.234
    
    // Test 3: Scale down with more than half - should round up
    let value3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12346u64), // 1.2346 with 4 decimals
        4
    );
    let result3 = tester.rescale_half_up(&value3, 3);
    assert_eq!(result3.into_raw_units(), &BigUint::from(1235u64)); // 1.235
    
    // Test 4: Scale up - should add zeros
    let value4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(123u64), // 1.23 with 2 decimals
        2
    );
    let result4 = tester.rescale_half_up(&value4, 5);
    assert_eq!(result4.into_raw_units(), &BigUint::from(123000u64)); // 1.23000
    
    // Test 5: Same scale - should return same value
    let value5 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12345u64),
        5
    );
    let result5 = tester.rescale_half_up(&value5, 5);
    assert_eq!(result5.into_raw_units(), &BigUint::from(12345u64));
    
    // Test 6: Scale down to 0 decimals
    let value6 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(12500u64), // 12.500 with 3 decimals
        3
    );
    let result6 = tester.rescale_half_up(&value6, 0);
    assert_eq!(result6.into_raw_units(), &BigUint::from(13u64)); // 13
    
    // Test 7: Very small number rounds to zero
    let value7 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(4u64), // 0.004 with 3 decimals
        3
    );
    let result7 = tester.rescale_half_up(&value7, 2);
    assert_eq!(result7.into_raw_units(), &BigUint::from(0u64)); // 0.00
    
    // Test 8: Edge case - 0.005 rounds to 0.01
    let value8 = ManagedDecimal::<StaticApi, usize>::from_raw_units(
        BigUint::from(5u64), // 0.005 with 3 decimals
        3
    );
    let result8 = tester.rescale_half_up(&value8, 2);
    assert_eq!(result8.into_raw_units(), &BigUint::from(1u64)); // 0.01
}

// ============== MUL_HALF_UP TESTS ==============

#[test]
fn test_mul_half_up_comprehensive() {
    let tester = MathTester;
    
    // Test 1: Basic multiplication with same precision
    let a1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(150u64), 2); // 1.50
    let b1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(200u64), 2); // 2.00
    let result1 = tester.mul_half_up(&a1, &b1, 2);
    assert_eq!(result1.into_raw_units(), &BigUint::from(300u64)); // 3.00
    
    // Test 2: Multiplication requiring rounding up
    let a2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(15u64), 1); // 1.5
    let b2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(17u64), 1); // 1.7
    let result2 = tester.mul_half_up(&a2, &b2, 1);
    assert_eq!(result2.into_raw_units(), &BigUint::from(26u64)); // 2.6 (2.55 rounds up)
    
    // Test 3: Multiplication with different input precisions
    let a3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1234u64), 3); // 1.234
    let b3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(56u64), 1); // 5.6
    let result3 = tester.mul_half_up(&a3, &b3, 2);
    // Note: inputs are rescaled to target precision first: 1.23 * 5.6 = 6.888, rounds to 6.89
    assert_eq!(result3.into_raw_units(), &BigUint::from(689u64)); // 6.89
    
    // Test 4: Very small numbers
    let a4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1u64), 3); // 0.001
    let b4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(5u64), 3); // 0.005
    let result4 = tester.mul_half_up(&a4, &b4, 6);
    assert_eq!(result4.into_raw_units(), &BigUint::from(5u64)); // 0.000005
    
    // Test 5: Multiply by zero
    let a5 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1234u64), 3);
    let b5 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(0u64), 3);
    let result5 = tester.mul_half_up(&a5, &b5, 3);
    assert_eq!(result5.into_raw_units(), &BigUint::from(0u64));
    
    // Test 6: Result precision higher than inputs
    let a6 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(33u64), 1); // 3.3
    let b6 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(33u64), 1); // 3.3
    let result6 = tester.mul_half_up(&a6, &b6, 3);
    assert_eq!(result6.into_raw_units(), &BigUint::from(10890u64)); // 10.890
    
    // Test 7: Exact half case
    let a7 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(25u64), 1); // 2.5
    let b7 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(25u64), 1); // 2.5
    let result7 = tester.mul_half_up(&a7, &b7, 1);
    assert_eq!(result7.into_raw_units(), &BigUint::from(63u64)); // 6.3 (6.25 rounds up)
}

// ============== DIV_HALF_UP TESTS ==============

#[test]
fn test_div_half_up_comprehensive() {
    let tester = MathTester;
    
    // Test 1: Basic division with no remainder
    let a1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(600u64), 2); // 6.00
    let b1 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(200u64), 2); // 2.00
    let result1 = tester.div_half_up(&a1, &b1, 2);
    assert_eq!(result1.into_raw_units(), &BigUint::from(300u64)); // 3.00
    
    // Test 2: Division with rounding up
    let a2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(100u64), 2); // 1.00
    let b2 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(300u64), 2); // 3.00
    let result2 = tester.div_half_up(&a2, &b2, 2);
    assert_eq!(result2.into_raw_units(), &BigUint::from(33u64)); // 0.33 (0.333... rounds down)
    
    // Test 3: Division with exact half - rounds up
    let a3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(50u64), 1); // 5.0
    let b3 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(20u64), 1); // 2.0
    let result3 = tester.div_half_up(&a3, &b3, 1);
    assert_eq!(result3.into_raw_units(), &BigUint::from(25u64)); // 2.5
    
    // Test 4: Division resulting in very small number
    let a4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1u64), 3); // 0.001
    let b4 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1000u64), 3); // 1.000
    let result4 = tester.div_half_up(&a4, &b4, 6);
    assert_eq!(result4.into_raw_units(), &BigUint::from(1000u64)); // 0.001000
    
    // Test 5: Division by 1
    let a5 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1234u64), 3); // 1.234
    let b5 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1000u64), 3); // 1.000
    let result5 = tester.div_half_up(&a5, &b5, 3);
    assert_eq!(result5.into_raw_units(), &BigUint::from(1234u64)); // 1.234
    
    // Test 6: Large number divided by small number (with appropriate precision)
    let a6 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(1000000u64), 2); // 10000.00
    let b6 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(3u64), 2); // 0.03
    let result6 = tester.div_half_up(&a6, &b6, 2);
    // 10000.00 / 0.03 = 333333.33..., rounds to 333333.33
    assert_eq!(result6.into_raw_units(), &BigUint::from(33333333u64)); // 333333.33
    
    // Test 7: 2/3 = 0.667 (rounds up from 0.6666...)
    let a7 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(200u64), 2); // 2.00
    let b7 = ManagedDecimal::<StaticApi, usize>::from_raw_units(BigUint::from(300u64), 2); // 3.00
    let result7 = tester.div_half_up(&a7, &b7, 3);
    assert_eq!(result7.into_raw_units(), &BigUint::from(667u64)); // 0.667
}

// ============== MUL_HALF_UP_SIGNED TESTS ==============

#[test]
fn test_mul_half_up_signed_comprehensive() {
    let tester = MathTester;
    
    // Test 1: Positive * Positive
    let a1 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(15i64), 1); // 1.5
    let b1 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(20i64), 1); // 2.0
    let result1 = tester.mul_half_up_signed(&a1, &b1, 1);
    assert_eq!(result1.into_raw_units(), &BigInt::from(30i64)); // 3.0
    
    // Test 2: Negative * Positive (with rounding)
    let a2 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-15i64), 1); // -1.5
    let b2 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(17i64), 1); // 1.7
    let result2 = tester.mul_half_up_signed(&a2, &b2, 1);
    assert_eq!(result2.into_raw_units(), &BigInt::from(-26i64)); // -2.6 (rounds away from zero)
    
    // Test 3: Negative * Negative
    let a3 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-25i64), 1); // -2.5
    let b3 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-30i64), 1); // -3.0
    let result3 = tester.mul_half_up_signed(&a3, &b3, 1);
    assert_eq!(result3.into_raw_units(), &BigInt::from(75i64)); // 7.5
    
    // Test 4: Exact half case with negative result
    let a4 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-25i64), 1); // -2.5
    let b4 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(25i64), 1); // 2.5
    let result4 = tester.mul_half_up_signed(&a4, &b4, 1);
    assert_eq!(result4.into_raw_units(), &BigInt::from(-63i64)); // -6.3 (rounds away from zero)
    
    // Test 5: Multiply by zero
    let a5 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-123i64), 2);
    let b5 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(0i64), 2);
    let result5 = tester.mul_half_up_signed(&a5, &b5, 2);
    assert_eq!(result5.into_raw_units(), &BigInt::from(0i64));
    
    // Test 6: Very small negative numbers
    let a6 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-1i64), 3); // -0.001
    let b6 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(5i64), 3); // 0.005
    let result6 = tester.mul_half_up_signed(&a6, &b6, 6);
    assert_eq!(result6.into_raw_units(), &BigInt::from(-5i64)); // -0.000005
    
    // Test 7: Rounding with different signs
    let a7 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(33i64), 1); // 3.3
    let b7 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-23i64), 1); // -2.3
    let result7 = tester.mul_half_up_signed(&a7, &b7, 1);
    assert_eq!(result7.into_raw_units(), &BigInt::from(-76i64)); // -7.6 (7.59 rounds away from zero)
}

// ============== DIV_HALF_UP_SIGNED TESTS ==============

#[test]
fn test_div_half_up_signed_comprehensive() {
    let tester = MathTester;
    
    // Test 1: Positive / Positive
    let a1 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(50i64), 1); // 5.0
    let b1 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(20i64), 1); // 2.0
    let result1 = tester.div_half_up_signed(&a1, &b1, 1);
    assert_eq!(result1.into_raw_units(), &BigInt::from(25i64)); // 2.5
    
    // Test 2: Negative / Positive with rounding
    let a2 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-100i64), 2); // -1.00
    let b2 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(300i64), 2); // 3.00
    let result2 = tester.div_half_up_signed(&a2, &b2, 2);
    assert_eq!(result2.into_raw_units(), &BigInt::from(-33i64)); // -0.33
    
    // Test 3: Negative / Negative
    let a3 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-60i64), 1); // -6.0
    let b3 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-20i64), 1); // -2.0
    let result3 = tester.div_half_up_signed(&a3, &b3, 1);
    assert_eq!(result3.into_raw_units(), &BigInt::from(30i64)); // 3.0
    
    // Test 4: Positive / Negative with exact half
    let a4 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(50i64), 1); // 5.0
    let b4 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-30i64), 1); // -3.0
    let result4 = tester.div_half_up_signed(&a4, &b4, 1);
    assert_eq!(result4.into_raw_units(), &BigInt::from(-17i64)); // -1.7 (rounds away from zero)
    
    // Test 5: Division by -1
    let a5 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(1234i64), 3); // 1.234
    let b5 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-1000i64), 3); // -1.000
    let result5 = tester.div_half_up_signed(&a5, &b5, 3);
    assert_eq!(result5.into_raw_units(), &BigInt::from(-1234i64)); // -1.234
    
    // Test 6: -2/3 = -0.667 (rounds away from zero)
    let a6 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-200i64), 2); // -2.00
    let b6 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(300i64), 2); // 3.00
    let result6 = tester.div_half_up_signed(&a6, &b6, 3);
    assert_eq!(result6.into_raw_units(), &BigInt::from(-667i64)); // -0.667
    
    // Test 7: Very small negative divided by large positive
    let a7 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(-1i64), 3); // -0.001
    let b7 = ManagedDecimalSigned::<StaticApi, usize>::from_raw_units(BigInt::from(2000i64), 3); // 2.000
    let result7 = tester.div_half_up_signed(&a7, &b7, 4);
    assert_eq!(result7.into_raw_units(), &BigInt::from(-5i64)); // -0.0005
}