#![no_std]

use core::cmp::Ordering;

use common_constants::{BPS, BPS_PRECISION, DOUBLE_RAY, RAY, RAY_PRECISION, WAD, WAD_PRECISION};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait SharedMathModule {
    /// Multiplies two decimals with half-up rounding at target precision.
    /// Prevents precision loss in financial calculations using banker's rounding.
    /// Returns product rounded to specified precision.
    fn mul_half_up(
        &self,
        a: &ManagedDecimal<Self::Api, NumDecimals>,
        b: &ManagedDecimal<Self::Api, NumDecimals>,
        precision: NumDecimals,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Use target precision directly, no +1
        let scaled_a = a.rescale(precision);
        let scaled_b = b.rescale(precision);

        // Perform multiplication in BigUint
        let product = scaled_a.into_raw_units() * scaled_b.into_raw_units();

        // Half-up rounding at precision
        let scaled = BigUint::from(10u64).pow(precision as u32);
        let half_scaled = &scaled / &BigUint::from(2u64);

        // Round half-up
        let rounded_product = (product + half_scaled) / scaled;

        self.to_decimal(rounded_product, precision)
    }

    /// Divides two decimals with half-up rounding at target precision.
    /// Prevents precision loss in financial calculations using banker's rounding.
    /// Returns quotient rounded to specified precision.
    fn div_half_up(
        &self,
        a: &ManagedDecimal<Self::Api, NumDecimals>,
        b: &ManagedDecimal<Self::Api, NumDecimals>,
        precision: NumDecimals,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Use target precision directly, no +1
        let scaled_a = a.rescale(precision);
        let scaled_b = b.rescale(precision);

        // Perform division in BigUint
        let scaled = BigUint::from(10u64).pow(precision as u32);
        let numerator = scaled_a.into_raw_units() * &scaled;
        let denominator = scaled_b.into_raw_units();

        // Half-up rounding
        let half_denominator = denominator / &BigUint::from(2u64);
        let rounded_quotient = (numerator + half_denominator) / denominator;

        self.to_decimal(rounded_quotient, precision)
    }

    /// Multiplies two signed decimals with half-up rounding away from zero.
    /// Handles negative values correctly for financial calculations.
    /// Returns signed product rounded to specified precision.
    fn mul_half_up_signed(
        &self,
        a: &ManagedDecimalSigned<Self::Api, NumDecimals>,
        b: &ManagedDecimalSigned<Self::Api, NumDecimals>,
        precision: NumDecimals,
    ) -> ManagedDecimalSigned<Self::Api, NumDecimals> {
        // Use target precision directly, no +1
        let scaled_a = a.rescale(precision);
        let scaled_b = b.rescale(precision);

        // Perform multiplication in BigUint
        let product = scaled_a.into_raw_units() * scaled_b.into_raw_units();

        // Half-up rounding at precision
        let scaled = BigInt::from(10i64).pow(precision as u32);
        let half_scaled = &scaled / &BigInt::from(2i64);

        // ─── sign-aware “away-from-zero” rounding ───────────────────────────
        let rounded_product = if product.sign() == Sign::Minus {
            // pull the value farther *below* zero
            (product - half_scaled) / scaled // truncates toward-0 ⇒ away-from-0
        } else {
            // push the value farther *above* zero
            (product + half_scaled) / scaled
        };

        ManagedDecimalSigned::from_raw_units(rounded_product, precision)
    }

    /// Divides two signed decimals with half-up rounding away from zero.
    /// Handles negative values correctly for financial calculations.
    /// Returns signed quotient rounded to specified precision.
    fn div_half_up_signed(
        &self,
        a: &ManagedDecimalSigned<Self::Api, NumDecimals>,
        b: &ManagedDecimalSigned<Self::Api, NumDecimals>,
        precision: NumDecimals,
    ) -> ManagedDecimalSigned<Self::Api, NumDecimals> {
        // Use target precision directly, no +1
        let scaled_a = a.rescale(precision);
        let scaled_b = b.rescale(precision);

        // Perform division in BigUint
        let scaled = BigInt::from(10i64).pow(precision as u32);
        let numerator = scaled_a.into_raw_units() * &scaled;
        let denominator = scaled_b.into_raw_units();

        // Half-up rounding
        let half_denominator = denominator / &BigInt::from(2i64);

        let sign_neg = numerator.sign() != denominator.sign();

        let rounded = if sign_neg {
            &(numerator - half_denominator) / denominator
        } else {
            &(numerator + half_denominator) / denominator
        };

        ManagedDecimalSigned::from_raw_units(rounded, precision)
    }

    /// Converts BigUint to ManagedDecimal with WAD precision (18 decimals).
    /// Standard precision for asset amounts in the protocol.
    fn to_decimal_wad(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, WAD_PRECISION)
    }

    /// Returns zero value with BPS precision (4 decimals).
    /// Used for basis point calculations.
    fn bps_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_bps(BigUint::zero())
    }

    /// Returns zero value with WAD precision (18 decimals).
    /// Used for asset amount calculations.
    fn wad_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_wad(BigUint::zero())
    }

    /// Returns zero value with RAY precision (27 decimals).
    /// Used for high-precision interest rate calculations.
    fn ray_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_ray(BigUint::zero())
    }

    /// Converts BigUint to ManagedDecimal with RAY precision (27 decimals).
    /// High precision for interest rate and index calculations.
    fn to_decimal_ray(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, RAY_PRECISION)
    }

    /// Converts BigUint to ManagedDecimal with BPS precision (4 decimals).
    /// Used for percentage values in basis points.
    fn to_decimal_bps(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, BPS_PRECISION)
    }

    /// Returns one unit in RAY precision (1e27).
    /// Base unit for high-precision calculations.
    fn ray(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(RAY), RAY_PRECISION)
    }

    /// Returns two units in RAY precision (2e27).
    /// Used for specific mathematical operations.
    fn double_ray(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(DOUBLE_RAY), RAY_PRECISION)
    }

    /// Returns one unit in WAD precision (1e18).
    /// Base unit for asset amount calculations.
    fn wad(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(WAD), WAD_PRECISION)
    }

    /// Returns 100% in BPS precision (10000).
    /// Base unit for percentage calculations.
    fn bps(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(BPS), BPS_PRECISION)
    }

    /// Converts BigUint to ManagedDecimal with specified precision.
    /// Core conversion utility for the protocol.
    fn to_decimal(
        self,
        value: BigUint,
        precision: NumDecimals,
    ) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        ManagedDecimal::from_raw_units(value, precision)
    }

    /// Rescales decimal to new precision with half-up rounding.
    /// Handles both upscaling and downscaling with proper rounding.
    /// Critical for cross-precision calculations.
    fn rescale_half_up(
        &self,
        value: &ManagedDecimal<Self::Api, NumDecimals>,
        new_precision: NumDecimals,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let old_precision = value.scale();
        let raw_value = value.into_raw_units();

        match new_precision.cmp(&old_precision) {
            Ordering::Equal => value.clone(),
            Ordering::Less => {
                let precision_diff = old_precision - new_precision;
                let factor = BigUint::from(10u64).pow(precision_diff as u32);
                let half_factor = &factor / 2u64;

                let rounded_downscaled_value = (raw_value + &half_factor) / factor;
                return ManagedDecimal::from_raw_units(rounded_downscaled_value, new_precision);
            },
            Ordering::Greater => value.rescale(new_precision),
        }
    }

    /// Returns the smaller of two decimal values.
    /// Used for cap enforcement and safety checks.
    fn min(
        self,
        a: ManagedDecimal<Self::Api, NumDecimals>,
        b: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if a < b {
            a
        } else {
            b
        }
    }

    /// Returns the larger of two decimal values.
    /// Used for floor enforcement and selection logic.
    fn max(
        self,
        a: ManagedDecimal<Self::Api, NumDecimals>,
        b: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if a > b {
            a
        } else {
            b
        }
    }
}
