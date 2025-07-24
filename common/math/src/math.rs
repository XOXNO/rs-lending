#![no_std]

use core::cmp::Ordering;

use common_constants::{BPS, BPS_PRECISION, DOUBLE_RAY, RAY, RAY_PRECISION, WAD, WAD_PRECISION};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait SharedMathModule {
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

    fn to_decimal_wad(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, WAD_PRECISION)
    }

    fn bps_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_bps(BigUint::zero())
    }

    fn wad_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_wad(BigUint::zero())
    }

    fn ray_zero(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal_ray(BigUint::zero())
    }

    fn to_decimal_ray(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, RAY_PRECISION)
    }

    fn to_decimal_bps(self, value: BigUint) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(value, BPS_PRECISION)
    }

    fn ray(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(RAY), RAY_PRECISION)
    }

    fn double_ray(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(DOUBLE_RAY), RAY_PRECISION)
    }

    fn wad(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(WAD), WAD_PRECISION)
    }

    fn bps(self) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        self.to_decimal(BigUint::from(BPS), BPS_PRECISION)
    }

    fn to_decimal(
        self,
        value: BigUint,
        precision: NumDecimals,
    ) -> ManagedDecimal<<Self as ContractBase>::Api, usize> {
        ManagedDecimal::from_raw_units(value, precision)
    }

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

    fn get_min(
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

    fn get_max(
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
