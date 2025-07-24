use common_constants::{
    BPS_PRECISION, K_SCALLING_FACTOR, MAX_FIRST_TOLERANCE, MAX_LAST_TOLERANCE,
    MAX_LIQUIDATION_BONUS, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE, RAY_PRECISION, WAD_PRECISION,
};
use common_errors::{
    ERROR_UNEXPECTED_ANCHOR_TOLERANCES, ERROR_UNEXPECTED_FIRST_TOLERANCE,
    ERROR_UNEXPECTED_LAST_TOLERANCE,
};
use common_structs::{OraclePriceFluctuation, PriceFeedShort};

multiversx_sc::imports!();

/// # Controller Math Helpers Module
///
/// This module provides critical mathematical functions for the MultiversX lending protocol,
/// implementing sophisticated financial calculations with high precision arithmetic.
///
/// ## Core Mathematical Functions
///
/// ### Health Factor Calculation
/// - **Formula**: `health_factor = weighted_collateral / borrowed_value`
/// - **Special Case**: Returns `u128::MAX` for positions with zero debt
/// - **Precision**: WAD (10^18) for final results
///
/// ### Dynamic Liquidation Bonus
/// - **Formula**: `bonus = min_bonus + (max_bonus - min_bonus) * min(k * gap, 1)`
/// - **Where**: `gap = (target_hf - current_hf) / target_hf`
/// - **Scaling**: Linear with k=200%, capped at 15% maximum bonus
///
/// ### Algebraic Liquidation Model
/// - **Formula**: `d_ideal = (target_hf * total_debt - weighted_collateral) / (target_hf - proportion_seized * (1 + bonus))`
/// - **Purpose**: Determines optimal debt repayment to achieve target health factor
/// - **Fallback**: Uses d_max when d_ideal is negative or denominator approaches zero
///
/// ### Dutch Auction Mechanism
/// - **Primary Target**: 1.02 WAD (102% health factor)
/// - **Secondary Target**: 1.01 WAD (101% health factor)
/// - **Purpose**: Progressive liquidation targeting based on position health
///
/// ## Precision Standards
/// - **RAY**: 10^27 precision for intermediate calculations
/// - **WAD**: 10^18 precision for final results and health factors
/// - **BPS**: 10^4 precision for percentages and bonuses
/// - **Rounding**: Half-up rounding used throughout for consistency
///
/// ## Security Considerations
/// - Oracle price tolerances validated within safe bounds
/// - Health factor calculations prevent division by zero
/// - Liquidation amounts capped to prevent over-liquidation
/// - Precision handling accounts for cross-token decimal differences
///
#[multiversx_sc::module]
pub trait MathsModule: common_math::SharedMathModule {
    /// Converts an EGLD amount to token units using the token's price feed data.
    ///
    /// **Purpose**: Normalizes EGLD values to token-specific decimals for cross-asset calculations
    /// in lending operations, ensuring consistent precision across different token types.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// token_amount = (amount_in_egld / token_price_in_egld) * 10^token_decimals
    /// ```
    ///
    /// **Process**:
    /// 1. Performs division in RAY precision (10^27) for maximum accuracy
    /// 2. Rescales result to token's native decimal precision using half-up rounding
    /// 3. Handles tokens with varying decimal places (e.g., USDC=6, WETH=18)
    ///
    /// **Security Considerations**:
    /// - Uses half-up rounding to prevent systematic precision loss
    /// - Maintains high precision throughout calculation before final rescaling
    /// - Handles edge cases where token decimals differ significantly from EGLD
    ///
    /// # Arguments
    /// - `amount_in_egld`: EGLD amount to convert (any precision)
    /// - `token_data`: Price feed data containing token price and decimal information
    ///
    /// # Returns
    /// - Token amount adjusted to the token's native decimal precision

    fn convert_egld_to_tokens(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.convert_egld_to_tokens_ray(amount_in_egld, token_data),
            token_data.asset_decimals,
        )
    }

    /// Converts an EGLD amount to token units in RAY precision for intermediate calculations.
    ///
    /// **Purpose**: Performs the core conversion calculation while maintaining maximum precision
    /// for downstream mathematical operations that require RAY precision.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// token_amount_ray = amount_in_egld / token_price_in_egld
    /// ```
    /// Result maintained in RAY precision (10^27) for chaining with other calculations.
    ///
    /// **Usage Pattern**: This function is typically used as an intermediate step
    /// in complex calculations where the result will undergo further mathematical
    /// operations before final rescaling.
    ///
    /// # Arguments
    /// - `amount_in_egld`: EGLD amount to convert (any precision)
    /// - `token_data`: Price feed data containing token price in EGLD terms
    ///
    /// # Returns
    /// - Token amount in RAY precision (10^27) for intermediate calculations

    fn convert_egld_to_tokens_ray(
        &self,
        amount_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        token_data: &PriceFeedShort<Self::Api>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.div_half_up(amount_in_egld, &token_data.price, RAY_PRECISION)
    }

    /// Computes the USD value of a token amount using its price.
    ///
    /// **Purpose**: Standardizes asset values in USD for collateral and borrow calculations,
    /// enabling cross-asset comparisons and risk assessments in the lending protocol.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// usd_value = token_amount * token_price_usd
    /// ```
    /// Calculated in RAY precision and rescaled to WAD for final result.
    ///
    /// **Process**:
    /// 1. Multiplies token amount by USD price in RAY precision
    /// 2. Rescales to WAD precision (10^18) using half-up rounding
    /// 3. Ensures consistent USD denomination across all protocol calculations
    ///
    /// **Usage**: Critical for determining total portfolio values, liquidation thresholds,
    /// and borrowing capacity calculations.
    ///
    /// # Arguments
    /// - `amount`: Token amount in its native decimal precision
    /// - `token_price`: USD price of the token (RAY precision)
    ///
    /// # Returns
    /// - USD value in WAD precision (10^18) for protocol-wide consistency

    fn get_egld_usd_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.mul_half_up(amount, token_price, RAY_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Computes the EGLD value of a token amount using its price.
    /// Facilitates internal calculations with EGLD as the base unit.
    ///
    /// # Arguments
    /// - `amount`: Token amount to convert.
    /// - `token_price`: EGLD price of the token.
    ///
    /// # Returns
    /// - EGLD value in WAD precision.

    fn get_token_egld_value(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.rescale_half_up(
            &self.mul_half_up(amount, token_price, RAY_PRECISION),
            WAD_PRECISION,
        )
    }

    /// Computes the EGLD value of a token amount using its price.
    ///
    /// **Purpose**: Facilitates internal calculations with EGLD as the base unit,
    /// enabling unified value calculations across the lending protocol's multi-asset system.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// egld_value = token_amount * token_price_in_egld
    /// ```
    /// Calculated in RAY precision and rescaled to WAD for standardization.
    ///
    /// **Process**:
    /// 1. Multiplies token amount by EGLD price in RAY precision
    /// 2. Rescales to WAD precision (10^18) using half-up rounding
    /// 3. Maintains EGLD as the common denominator for all value calculations
    ///
    /// **Usage**: Essential for health factor calculations, liquidation assessments,
    /// and cross-collateral evaluations where EGLD serves as the reference currency.
    ///
    /// # Arguments
    /// - `amount`: Token amount in its native decimal precision
    /// - `token_price`: EGLD price of the token (RAY precision)
    ///
    /// # Returns
    /// - EGLD value in WAD precision (10^18) for protocol standardization
    /// Computes the EGLD value of a token amount in RAY precision for intermediate calculations.
    ///
    /// **Purpose**: Provides EGLD value calculation while maintaining RAY precision
    /// for complex mathematical operations that require maximum accuracy.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// egld_value_ray = token_amount * token_price_in_egld
    /// ```
    /// Result maintained in RAY precision (10^27) without rescaling.
    ///
    /// **Usage Pattern**: Used in multi-step calculations where intermediate results
    /// need to maintain maximum precision before final output formatting.
    ///
    /// **Precision Handling**: Avoids precision loss from multiple rescaling operations
    /// by keeping the result in RAY precision for subsequent calculations.
    ///
    /// # Arguments
    /// - `amount`: Token amount in its native decimal precision
    /// - `token_price`: EGLD price of the token (RAY precision)
    ///
    /// # Returns
    /// - EGLD value in RAY precision (10^27) for intermediate calculations

    fn get_token_egld_value_ray(
        &self,
        amount: &ManagedDecimal<Self::Api, NumDecimals>,
        token_price: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        self.mul_half_up(amount, token_price, RAY_PRECISION)
    }

    /// Calculates the health factor from weighted collateral and borrowed value.
    ///
    /// **Purpose**: Assesses the risk level of a user's position in the lending protocol.
    /// The health factor is the primary metric for determining liquidation eligibility
    /// and position safety. Higher values indicate safer positions.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// health_factor = weighted_collateral / borrowed_value
    /// ```
    /// Where:
    /// - `weighted_collateral`: Collateral value multiplied by liquidation threshold
    /// - `borrowed_value`: Total debt value across all borrowed assets
    ///
    /// **Critical Thresholds**:
    /// - `health_factor >= 1.0`: Position is safe (above liquidation threshold)
    /// - `health_factor < 1.0`: Position is liquidatable
    /// - `health_factor = u128::MAX`: Position has no debt (infinitely safe)
    ///
    /// **Security Considerations**:
    /// - **Zero Division Protection**: Returns `u128::MAX` when borrowed_value is zero
    /// - **Precision Handling**: Calculated in RAY precision
    /// - **Overflow Protection**: Uses `u128::MAX` to represent infinite health factor
    ///
    /// **Economic Rationale**: The health factor represents how much the collateral
    /// value can decrease before reaching the liquidation threshold. A health factor
    /// of 1.5 means collateral can lose 33.3% of its value before liquidation.
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Collateral value weighted by liquidation thresholds (RAY)
    /// - `borrowed_value_in_egld`: Total borrowed value in EGLD (RAY)
    ///
    /// # Returns
    /// - Health factor in RAY precision (10^27); `u128::MAX` if no borrows exist
    fn compute_health_factor(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_value_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        if borrowed_value_in_egld == &self.ray_zero() {
            return self.double_ray();
        }
        self.div_half_up(
            weighted_collateral_in_egld,
            borrowed_value_in_egld,
            RAY_PRECISION,
        )
    }

    /// Calculates upper and lower bounds for a tolerance in basis points.
    ///
    /// **Purpose**: Determines acceptable price ranges for oracle price fluctuation checks,
    /// establishing symmetric bounds around the reference price for deviation validation.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// upper_bound = 10000 + tolerance (in BPS)
    /// lower_bound = 10000 / upper_bound * 10000
    /// ```
    /// This creates symmetric percentage bounds around 100% (10000 BPS).
    ///
    /// **Example**: For tolerance = 500 BPS (5%):
    /// - `upper_bound` = 10500 BPS (105%)
    /// - `lower_bound` = 9523 BPS (~95.23%)
    ///
    /// **Symmetry**: The bounds are mathematically symmetric around 100%,
    /// meaning a 5% increase and 5% decrease have equal tolerance weights.
    ///
    /// # Arguments
    /// - `tolerance`: Tolerance value in basis points (BPS precision, 10^4)
    ///
    /// # Returns
    /// - Tuple of (upper_bound, lower_bound) in BPS precision

    fn calculate_tolerance_range(
        &self,
        tolerance: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let upper = self.bps() + tolerance;
        let lower = self.div_half_up(&self.bps(), &upper, BPS_PRECISION);

        (upper, lower)
    }

    /// Validates and computes oracle price fluctuation tolerances.
    ///
    /// **Purpose**: Ensures price deviations stay within safe limits for oracle reliability,
    /// implementing a two-tier tolerance system for price feed validation.
    ///
    /// **Security Model**:
    /// - **First Tolerance**: Primary deviation threshold for normal operations
    /// - **Last Tolerance**: Maximum deviation threshold before price rejection
    /// - **Hierarchical Structure**: `first_tolerance <= last_tolerance`
    ///
    /// **Validation Rules**:
    /// ```
    /// MIN_FIRST_TOLERANCE <= first_tolerance <= MAX_FIRST_TOLERANCE
    /// MIN_LAST_TOLERANCE <= last_tolerance <= MAX_LAST_TOLERANCE
    /// first_tolerance <= last_tolerance
    /// ```
    ///
    /// **Security Considerations**:
    /// - **Bounded Tolerances**: Prevents excessive price deviation acceptance
    /// - **Ordered Validation**: Ensures logical progression from first to last tolerance
    /// - **Fail-Safe Design**: Rejects invalid tolerance configurations to maintain protocol safety
    ///
    /// **Economic Rationale**: Two-tier system allows for:
    /// 1. Early warning at first tolerance breach
    /// 2. Final validation at last tolerance before complete price rejection
    /// 3. Graceful degradation of oracle reliability assessment
    ///
    /// # Arguments
    /// - `first_tolerance`: Initial tolerance for price deviation (BPS)
    /// - `last_tolerance`: Maximum allowed tolerance (BPS)
    ///
    /// # Returns
    /// - `OraclePriceFluctuation` struct with calculated bounds for both tolerances
    ///
    /// # Panics
    /// - If tolerances are outside acceptable ranges
    /// - If `last_tolerance < first_tolerance`
    fn validate_and_calculate_tolerances(
        &self,
        first_tolerance: &BigUint,
        last_tolerance: &BigUint,
    ) -> OraclePriceFluctuation<Self::Api> {
        require!(
            first_tolerance >= &BigUint::from(MIN_FIRST_TOLERANCE)
                && first_tolerance <= &BigUint::from(MAX_FIRST_TOLERANCE),
            ERROR_UNEXPECTED_FIRST_TOLERANCE
        );
        require!(
            last_tolerance >= &BigUint::from(MIN_LAST_TOLERANCE)
                && last_tolerance <= &BigUint::from(MAX_LAST_TOLERANCE),
            ERROR_UNEXPECTED_LAST_TOLERANCE
        );
        require!(
            last_tolerance >= first_tolerance,
            ERROR_UNEXPECTED_ANCHOR_TOLERANCES
        );

        let (first_upper_ratio, first_lower_ratio) =
            self.calculate_tolerance_range(self.to_decimal_bps(first_tolerance.clone()));
        let (last_upper_ratio, last_lower_ratio) =
            self.calculate_tolerance_range(self.to_decimal_bps(last_tolerance.clone()));

        OraclePriceFluctuation {
            first_upper_ratio,
            first_lower_ratio,
            last_upper_ratio,
            last_lower_ratio,
        }
    }

    /// Calculates a linearly scaled liquidation bonus based on the health factor gap.
    ///
    /// **Purpose**: Implements a dynamic liquidation bonus system that scales proportionally
    /// with position risk, providing fair compensation to liquidators while protecting borrowers
    /// from excessive penalties.
    ///
    /// **Mathematical Formula**:
    /// ```
    /// gap = (target_hf - current_hf) / target_hf
    /// scaled_term = min(k * gap, 1.0)  // Clamped to [0, 1]
    /// bonus = min_bonus + (max_bonus - min_bonus) * scaled_term
    /// ```
    ///
    /// **Where**:
    /// - `gap`: Normalized health factor deficit (0 to 1)
    /// - `k`: Scaling factor = 200% (amplifies gap for bonus calculation)
    /// - `max_bonus`: Capped at 15% (1500 BPS) for borrower protection
    ///
    /// **Economic Rationale**:
    /// 1. **Risk-Proportional**: Higher risk positions offer larger liquidation incentives
    /// 2. **Bounded Rewards**: Minimum bonus ensures liquidator participation
    /// 3. **Maximum Protection**: 15% cap prevents excessive borrower penalties
    /// 4. **Linear Scaling**: Predictable bonus progression encourages timely liquidations
    ///
    /// **Scaling Examples**:
    /// - `current_hf = 0.95, target_hf = 1.01`: gap ≈ 5.9%, bonus ≈ min + 11.8% of range
    /// - `current_hf = 0.80, target_hf = 1.01`: gap ≈ 20.8%, bonus ≈ min + 41.6% of range
    /// - `current_hf = 0.50, target_hf = 1.01`: gap ≈ 50.5%, bonus = max (clamped)
    ///
    /// **Security Considerations**:
    /// - **Overflow Protection**: Clamping prevents bonus calculation overflow
    /// - **Reasonable Bounds**: 15% maximum protects borrowers from excessive liquidation costs
    /// - **Linear Predictability**: Prevents gaming through predictable bonus progression
    ///
    /// # Arguments
    /// - `current_hf`: Current health factor (RAY precision, 10^27)
    /// - `target_hf`: Target health factor post-liquidation (RAY precision, 10^27)
    /// - `min_bonus`: Minimum liquidation bonus (RAY precision, 10^27)
    ///
    /// # Returns
    /// - Liquidation bonus in RAY precision (10^27), range: [min_bonus, 1500]
    fn calculate_linear_bonus(
        &self,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        // Capped at 15%
        let max_bonus = self.to_decimal_bps(BigUint::from(MAX_LIQUIDATION_BONUS));

        // Scaling factor of 200%
        let k = self.to_decimal_bps(BigUint::from(K_SCALLING_FACTOR));

        // Calculate the health factor gap: (target_hf - current_hf) / target_hf
        let gap = self.div_half_up(
            &(target_hf.clone() - current_hf.clone()),
            target_hf,
            RAY_PRECISION,
        );
        // Calculate the scaled term: k * gap
        let scaled_term = self.mul_half_up(&k, &gap, RAY_PRECISION);
        // Clamp the scaled term between 0 and 1
        let clamped_term = self.get_min(scaled_term, self.ray());
        // Calculate the bonus range: max_bonus - min_bonus

        let bonus_range = max_bonus.rescale(RAY_PRECISION) - min_bonus.clone();

        // Calculate the bonus increment: bonus_range * clamped_term
        let bonus_increment = self.mul_half_up(&bonus_range, &clamped_term, RAY_PRECISION);
        // Final bonus: min_bonus + bonus_increment
        min_bonus.clone() + bonus_increment
    }

    /// Computes debt repayment, bonus, and new health factor for a liquidation.
    ///
    /// **Purpose**: Implements the core algebraic liquidation model that determines the optimal
    /// debt repayment amount to achieve a target health factor, balancing liquidator incentives
    /// with borrower protection.
    ///
    /// **Algebraic Liquidation Formula**:
    /// ```
    /// d_ideal = (target_hf * total_debt - weighted_collateral) /
    ///           (target_hf - proportion_seized * (1 + bonus))
    /// ```
    ///
    /// **Where**:
    /// - `d_ideal`: Optimal debt repayment to achieve target health factor
    /// - `target_hf`: Desired post-liquidation health factor
    /// - `proportion_seized`: Fraction of collateral seized per unit debt repaid
    /// - `bonus`: Liquidation bonus rate (additional reward for liquidator)
    ///
    /// **Mathematical Derivation**:
    /// Starting from the target health factor equation:
    /// ```
    /// target_hf = (weighted_collateral - seized_weighted) / (total_debt - d)
    ///
    /// Where: seized_weighted = d * proportion_seized * (1 + bonus)
    ///
    /// Solving for d:
    /// target_hf * (total_debt - d) = weighted_collateral - d * proportion_seized * (1 + bonus)
    /// target_hf * total_debt - target_hf * d = weighted_collateral - d * proportion_seized * (1 + bonus)
    /// target_hf * total_debt - weighted_collateral = target_hf * d - d * proportion_seized * (1 + bonus)
    /// target_hf * total_debt - weighted_collateral = d * (target_hf - proportion_seized * (1 + bonus))
    ///
    /// Therefore: d = (target_hf * total_debt - weighted_collateral) / (target_hf - proportion_seized * (1 + bonus))
    /// ```
    ///
    /// **Edge Case Handling**:
    /// 1. **Division by Zero**: When `target_hf == proportion_seized * (1 + bonus)`, uses `d_max`
    /// 2. **Negative d_ideal**: When position cannot be made healthy, uses `d_max` (full liquidation)
    /// 3. **Excess Liquidation**: `d_ideal` is capped at `d_max` to prevent over-liquidation
    ///
    /// **Where d_max**:
    /// ```
    /// d_max = total_collateral / (1 + bonus)
    /// ```
    /// Represents the maximum possible debt repayment given available collateral.
    ///
    /// **Security Considerations**:
    /// - **Overflow Protection**: Uses signed arithmetic for intermediate calculations
    /// - **Precision Handling**: Maintains RAY precision throughout complex calculations
    /// - **Boundary Validation**: Ensures `d_ideal` never exceeds available collateral
    /// - **Graceful Degradation**: Falls back to maximum liquidation when ideal is impossible
    ///
    /// **Economic Properties**:
    /// - **Optimal Liquidation**: Minimizes liquidation amount while achieving target health
    /// - **Proportional Scaling**: Larger health deficits require proportionally larger liquidations
    /// - **Incentive Alignment**: Balances liquidator rewards with borrower protection
    ///
    /// # Arguments
    /// - `total_collateral`: Total collateral value (RAY precision)
    /// - `weighted_collateral`: Collateral value weighted by liquidation thresholds (RAY)
    /// - `proportion_seized`: Proportion of collateral seized per unit debt (BPS precision)
    /// - `liquidation_bonus`: Liquidation bonus rate (BPS precision)
    /// - `total_debt`: Total debt value (RAY precision)
    /// - `target_hf`: Target post-liquidation health factor (RAY precision)
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, liquidation_bonus, new_health_factor) in appropriate precisions
    fn compute_liquidation_details(
        &self,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Constants
        let bps = self.bps();

        // Convert to signed for intermediate calculations
        let total_debt_ray_signed = total_debt.clone().into_signed();
        let target_health = target_hf.clone().into_signed();
        let weighted_collateral_ray = weighted_collateral.clone().into_signed();

        // Compute 1 + b
        let one_plus_bonus = self.bps() + liquidation_bonus.clone();
        let d_max = self.div_half_up(
            &self.mul_half_up(total_collateral, &bps, RAY_PRECISION),
            &one_plus_bonus,
            RAY_PRECISION,
        );

        let denominator_term = self
            .mul_half_up(proportion_seized, &one_plus_bonus, RAY_PRECISION)
            .into_signed();

        // Avoid edge case where target_health == denominator_term which would result in division by zero
        let debt_to_repay_ray = if target_health == denominator_term {
            d_max
        } else {
            // Compute d_ideal
            let numerator_term =
                self.mul_half_up_signed(&target_health, &total_debt_ray_signed, RAY_PRECISION);
            let numerator = numerator_term - weighted_collateral_ray;
            let denominator = target_health - denominator_term;
            let d_ideal = self.div_half_up_signed(&numerator, &denominator, RAY_PRECISION);

            // Determine debt_to_repay, will fall back to d_max if d_ideal is negative since it's not possible to be heatlhy anymore
            if d_ideal.sign() == Sign::Minus {
                d_max
            } else {
                self.get_min(d_ideal.into_unsigned_or_fail(), d_max)
            }
        };

        // Calculate new health factor
        let new_health_factor = self.calculate_post_liquidation_health_factor(
            weighted_collateral,
            total_debt,
            &debt_to_repay_ray,
            proportion_seized,
            liquidation_bonus,
        );

        (
            debt_to_repay_ray,
            liquidation_bonus.clone(),
            new_health_factor,
        )
    }

    /// Estimates optimal debt repayment and bonus for liquidation using Dutch auction mechanism.
    ///
    /// **Purpose**: Implements a sophisticated Dutch auction system that progressively targets
    /// different health factor levels, optimizing liquidation outcomes for both liquidators
    /// and borrowers through adaptive target selection.
    ///
    /// **Dutch Auction Algorithm**:
    /// ```
    /// 1. Primary Target: 1.02 WAD (102% health factor)
    ///    - Attempt liquidation targeting 102% health factor
    ///    - If achievable (new_hf >= 1.0), return this result
    ///
    /// 2. Secondary Target: 1.01 WAD (101% health factor)
    ///    - If primary target fails to achieve safe health factor
    ///    - Fall back to more aggressive 101% target
    ///    - Return result regardless of final health factor
    /// ```
    ///
    /// **Target Health Factor Rationale**:
    ///
    /// **Primary Target (1.02 WAD - 102%)**:
    /// - **Conservative Approach**: Provides 2% safety buffer above liquidation threshold
    /// - **Borrower Protection**: Minimizes liquidation amount while ensuring safety
    /// - **Market Stability**: Reduces likelihood of cascading liquidations
    /// - **Gas Efficiency**: Lower liquidation amounts reduce transaction costs
    ///
    /// **Secondary Target (1.01 WAD - 101%)**:
    /// - **Aggressive Recovery**: Used when conservative approach is insufficient
    /// - **Liquidator Incentive**: Larger liquidations provide better compensation
    /// - **Risk Mitigation**: Ensures positions are moved away from liquidation threshold
    /// - **Protocol Safety**: Guarantees meaningful health factor improvement
    ///
    /// **Economic Benefits**:
    /// 1. **Adaptive Liquidation**: Automatically adjusts liquidation size based on position health
    /// 2. **Optimal Efficiency**: Minimizes liquidation when possible, maximizes when necessary
    /// 3. **Market Responsive**: Targets adapt to varying market conditions and position risk
    /// 4. **Incentive Balanced**: Provides fair compensation while protecting borrowers
    ///
    /// **Algorithm Flow**:
    /// ```
    /// if simulate_liquidation(target=1.02) results in health_factor >= 1.0:
    ///     return conservative_liquidation  // Minimal, safe liquidation
    /// else:
    ///     return aggressive_liquidation    // Larger liquidation with 1.01 target
    /// ```
    ///
    /// **Use Cases**:
    /// - **Healthy Positions**: Use 1.02 target for minimal disruption
    /// - **Risky Positions**: Use 1.01 target for substantial health improvement
    /// - **Edge Cases**: Graceful handling of positions near liquidation boundary
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Weighted collateral value in EGLD (RAY)
    /// - `proportion_seized`: Proportion of collateral seized per unit debt (BPS)
    /// - `total_collateral`: Total collateral value (RAY)
    /// - `total_debt`: Total debt value (RAY)
    /// - `min_bonus`: Minimum liquidation bonus (BPS)
    /// - `current_hf`: Current health factor (RAY)
    ///
    /// # Returns
    /// - Tuple of (optimal_debt_to_repay, calculated_bonus) in appropriate precisions
    fn estimate_liquidation_amount(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let ray = self.ray();

        let target_best = ray.clone().into_raw_units() / 50u32 + ray.into_raw_units(); // 1.02 RAY

        let (safest_debt, safest_bonus, safe_new_hf) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            self.to_decimal_ray(target_best),
        );

        if safe_new_hf >= self.ray() {
            return (safest_debt, safest_bonus);
        }

        let target_best_second = ray.clone().into_raw_units() / 100u32 + ray.into_raw_units(); // 1.01 RAY
        let (limit_debt, limit_bonus, _) = self.simulate_liquidation(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral,
            total_debt,
            min_bonus,
            current_hf,
            self.to_decimal_ray(target_best_second),
        );

        (limit_debt, limit_bonus)
    }

    /// Calculates the new health factor after a liquidation operation.
    ///
    /// **Purpose**: Simulates the health factor that would result from a liquidation
    /// with the given parameters, providing critical validation for liquidation safety
    /// and accuracy estimation.
    ///
    /// **Mathematical Model**:
    /// ```
    /// seized_value = debt_to_repay * proportion_seized * (1 + bonus)
    /// seized_weighted = min(seized_value, weighted_collateral)
    ///
    /// new_weighted_collateral = weighted_collateral - seized_weighted
    /// new_total_debt = max(0, total_debt - debt_to_repay)
    ///
    /// new_health_factor = new_weighted_collateral / new_total_debt
    /// ```
    ///
    /// **Precision Handling Details**:
    ///
    /// This calculation works in EGLD terms with high precision (RAY/WAD).
    /// **Important**: The actual health factor after liquidation may be slightly lower
    /// (typically ~0.1-0.3%) due to rounding when converting between EGLD and individual
    /// token amounts with different decimal precisions.
    ///
    /// This precision variance is:
    /// 1. **Expected Behavior**: Inherent to cross-token decimal conversion
    /// 2. **Safely Conservative**: Ensures positions remain above 1.0 health factor
    /// 3. **Economically Insignificant**: Variance is typically < 0.3%
    /// 4. **Liquidator Friendly**: Slight under-estimation protects against failed liquidations
    ///
    /// **Security Calculations**:
    /// - **Collateral Bounds**: `seized_weighted` is capped at available weighted collateral
    /// - **Debt Bounds**: New debt cannot be negative (handles over-repayment)
    /// - **Division Safety**: Inherits zero-division protection from `compute_health_factor`
    /// - **Precision Consistency**: Maintains consistent precision scaling throughout
    ///
    /// **Economic Validation**:
    /// This function serves as a critical validation step, ensuring that:
    /// 1. Liquidations achieve the intended health factor improvement
    /// 2. Collateral seizure amounts are economically justified
    /// 3. Post-liquidation positions remain within safe parameters
    ///
    /// **Usage Pattern**: Called after `compute_liquidation_details` to validate
    /// the calculated liquidation parameters before execution.
    ///
    /// # Arguments
    /// - `weighted_collateral_ray`: Current weighted collateral value (RAY precision)
    /// - `total_debt_ray`: Current total debt value (RAY precision)
    /// - `debt_to_repay`: Amount of debt being repaid (RAY precision)
    /// - `proportion_seized`: Proportion of collateral seized per unit debt (BPS precision)
    /// - `liquidation_bonus`: Liquidation bonus rate (BPS precision)
    ///
    /// # Returns
    /// - New health factor after liquidation (WAD precision)
    /// - Returns `u128::MAX` if new debt becomes zero
    fn calculate_post_liquidation_health_factor(
        &self,
        weighted_collateral_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_repay_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let one_plus_bonus = self.bps() + liquidation_bonus.clone();

        // Compute seized_weighted
        let seized = self.mul_half_up(proportion_seized, debt_to_repay_ray, RAY_PRECISION);
        let seized_weighted_raw = self.mul_half_up(&seized, &one_plus_bonus, RAY_PRECISION);
        let seized_weighted = self.get_min(seized_weighted_raw, weighted_collateral_ray.clone());
        // Compute new weighted collateral and total debt
        let new_weighted_ray = weighted_collateral_ray.clone() - seized_weighted;
        let new_total_debt_ray = if debt_to_repay_ray >= total_debt_ray {
            self.ray_zero()
        } else {
            total_debt_ray.clone() - debt_to_repay_ray.clone()
        };
        // Compute new_health_factor
        self.compute_health_factor(&new_weighted_ray, &new_total_debt_ray)
    }

    /// Simulates a liquidation to estimate debt repayment, bonus, and new health factor.
    ///
    /// **Purpose**: Provides the core simulation engine for the Dutch auction mechanism,
    /// combining dynamic bonus calculation with algebraic liquidation modeling to
    /// produce comprehensive liquidation scenarios.
    ///
    /// **Simulation Process**:
    /// ```
    /// 1. Calculate dynamic bonus based on health factor gap
    ///    bonus = calculate_linear_bonus(current_hf, target_hf, min_bonus)
    ///
    /// 2. Compute optimal liquidation parameters
    ///    (debt_to_repay, _, new_hf) = compute_liquidation_details(...)
    ///
    /// 3. Return complete liquidation scenario
    ///    return (debt_to_repay, bonus, new_health_factor)
    /// ```
    ///
    /// **Integration with Dutch Auction**:
    /// This function serves as the simulation engine called by `estimate_liquidation_amount`
    /// with different target health factors (1.02 and 1.01 RAY) to implement the
    /// progressive targeting strategy.
    ///
    /// **Mathematical Consistency**:
    /// - Uses the same bonus calculation as actual liquidations
    /// - Applies identical algebraic liquidation formulas
    /// - Maintains precision consistency with execution path
    ///
    /// **Validation Purpose**:
    /// Results from this simulation are used to:
    /// 1. Validate liquidation feasibility before execution
    /// 2. Compare different target scenarios in the Dutch auction
    /// 3. Provide accurate estimates for liquidator decision-making
    ///
    /// # Arguments
    /// - `weighted_collateral_in_egld`: Weighted collateral value in EGLD (RAY)
    /// - `proportion_seized`: Proportion of collateral seized per unit debt (RAY)
    /// - `total_collateral`: Total collateral value (RAY)
    /// - `total_debt`: Total debt value (RAY)
    /// - `min_bonus`: Minimum liquidation bonus (RAY)
    /// - `current_hf`: Current health factor (RAY)
    /// - `target_hf`: Target post-liquidation health factor (RAY)
    ///
    /// # Returns
    /// - Tuple of (debt_to_repay, bonus, simulated_new_health_factor) in appropriate precisions
    fn simulate_liquidation(
        &self,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        total_debt: &ManagedDecimal<Self::Api, NumDecimals>,
        min_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        current_hf: &ManagedDecimal<Self::Api, NumDecimals>,
        target_hf: ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let bonus = self.calculate_linear_bonus(current_hf, &target_hf, min_bonus);

        self.compute_liquidation_details(
            total_collateral,
            weighted_collateral_in_egld,
            proportion_seized,
            &bonus,
            total_debt,
            target_hf,
        )
    }
}
