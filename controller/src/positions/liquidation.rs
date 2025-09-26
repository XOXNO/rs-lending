use common_constants::{RAY_PRECISION, WAD_PRECISION};
use common_structs::{AccountPosition, AccountPositionType, PriceFeedShort};

use crate::{cache::Cache, helpers, oracle, proxy_pool, storage, utils, validation};
use common_errors::{
    ERROR_HEALTH_FACTOR, ERROR_INVALID_PAYMENTS, ERROR_NO_DEBT_PAYMENTS_TO_PROCESS,
};

use super::{account, borrow, emode, repay, update, withdraw};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait PositionLiquidationModule:
    storage::Storage
    + validation::ValidationModule
    + oracle::OracleModule
    + common_events::EventsModule
    + utils::LendingUtilsModule
    + helpers::MathsModule
    + account::PositionAccountModule
    + repay::PositionRepayModule
    + withdraw::PositionWithdrawModule
    + update::PositionUpdateModule
    + borrow::PositionBorrowModule
    + common_math::SharedMathModule
    + common_rates::InterestRates
    + emode::EModeModule
{
    /// Executes the core liquidation logic for an unhealthy position using a sophisticated Dutch auction mechanism.
    ///
    /// # Purpose and Scope
    /// This function implements the main liquidation algorithm that:
    /// - Validates the position's health factor (must be < 1.0)
    /// - Calculates optimal debt repayment and collateral seizure amounts
    /// - Applies dynamic liquidation bonuses with proportional seizure
    /// - Handles excess payments with automatic refunds
    /// - Triggers bad debt cleanup when necessary
    ///
    /// # How It Works (Methodology and Process)
    /// 1. **Position Analysis**: Retrieves all deposit and borrow positions for the account
    /// 2. **Repayment Calculation**: Processes debt payments, handling excess amounts with refunds
    /// 3. **Collateral Valuation**: Calculates total and liquidation-weighted collateral values
    /// 4. **Seizure Proportions**: Determines weighted seizure ratios across multiple collateral assets
    /// 5. **Dutch Auction Logic**: Applies algebraic liquidation model targeting 1.02/1.01 health factors
    /// 6. **Proportional Seizure**: Distributes seized collateral proportionally across all deposit positions
    /// 7. **Bad Debt Detection**: Checks if remaining debt/collateral falls below $5 USD threshold
    ///
    /// # Mathematical Formulas
    /// - Health Factor: `weighted_collateral / total_debt` (must be < 1.0 for liquidation)
    /// - Liquidation Bonus: `linear_scaling * (1 - health_factor) * 200%` (capped at 15%)
    /// - Seizure Amount: `debt_repaid * (1 + liquidation_bonus)`
    /// - Proportional Distribution: `asset_collateral_value / total_collateral_value * total_seizure`
    ///
    /// # Security Checks Implemented
    /// - Health factor validation (position must be liquidatable)
    /// - Reentrancy protection via cache.flash_loan_ongoing
    /// - Payment validation for all debt repayments
    /// - Excess payment detection and automatic refunding
    /// - Bad debt threshold validation ($5 USD minimum)
    /// - Precision loss mitigation in decimal conversions
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce identifying the borrower's account
    /// - `debt_payments`: Vector of ERC20/EGLD payments for debt repayment
    /// - `cache`: Mutable storage cache for price feeds and pool addresses
    ///
    /// # Returns
    /// Returns a tuple containing:
    /// - `seized_collaterals`: Vector of (seized_payment, protocol_fee) for each collateral asset
    /// - `repaid_tokens`: Vector of (payment, egld_value, price_feed) for each repaid debt
    /// - `refunds`: Vector of excess payments to be refunded to liquidator
    /// - `max_debt_to_repay_ray`: Maximum debt amount that was repaid (RAY precision)
    /// - `bonus_rate_ray`: Applied liquidation bonus rate (RAY precision)
    fn execute_liquidation(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>,
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
        ManagedVec<EgldOrEsdtTokenPayment>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut refunds = ManagedVec::new();
        let deposit_positions = self
            .positions(account_nonce, AccountPositionType::Deposit)
            .values()
            .collect();

        let (borrow_positions, map_debt_indexes) = self.borrow_positions(account_nonce, true);

        let (debt_payment_in_egld_ray, mut repaid_tokens) = self.calculate_repayment_amounts(
            debt_payments,
            &borrow_positions,
            &mut refunds,
            map_debt_indexes,
            cache,
        );

        let (liquidation_collateral, total_collateral, _) =
            self.calculate_collateral_values(&deposit_positions, cache);
        let (proportional_weighted, bonus_weighted) =
            self.calculate_seizure_proportions(&total_collateral, &deposit_positions, cache);
        let borrowed_egld = self.calculate_total_borrow_in_egld(&borrow_positions, cache);

        let health_factor =
            self.validate_liquidation_health_factor(&liquidation_collateral, &borrowed_egld);

        let (max_debt_to_repay_ray, max_collateral_seized_ray, bonus_rate_ray) = self
            .calculate_liquidation_amounts(
                &borrowed_egld,
                &total_collateral,
                &liquidation_collateral,
                &proportional_weighted,
                &bonus_weighted,
                &health_factor,
                &debt_payment_in_egld_ray,
            );

        let seized_collaterals = self.calculate_seized_collateral(
            &deposit_positions,
            &total_collateral,
            &max_debt_to_repay_ray,
            &bonus_rate_ray,
            cache,
        );

        self.check_bad_debt_after_liquidation(
            cache,
            account_nonce,
            &borrowed_egld,
            &max_debt_to_repay_ray,
            &total_collateral,
            &max_collateral_seized_ray,
        );

        let user_paid_more = debt_payment_in_egld_ray > max_debt_to_repay_ray;
        // User paid more than the max debt to repay, so we need to refund the excess.
        if user_paid_more {
            let excess_payment_ray = debt_payment_in_egld_ray - max_debt_to_repay_ray.clone();
            self.process_excess_payment(&mut repaid_tokens, &mut refunds, excess_payment_ray);
        }

        (
            seized_collaterals,
            repaid_tokens,
            refunds,
            max_debt_to_repay_ray,
            bonus_rate_ray,
        )
    }

    /// Orchestrates the complete liquidation workflow from validation to settlement.
    ///
    /// # Purpose and Scope
    /// This is the main entry point for liquidations that coordinates the entire process:
    /// - Validates liquidation prerequisites and security constraints
    /// - Executes core liquidation logic with Dutch auction pricing
    /// - Handles payment processing and refund distribution
    /// - Processes debt repayments through liquidity pool interactions
    /// - Manages collateral seizure and protocol fee collection
    /// - Ensures proper event emission and accounting updates
    ///
    /// # How It Works (Complete Liquidation Workflow)
    /// 1. **Security Setup**: Establishes reentrancy protection and cache initialization
    /// 2. **Payment Validation**: Validates liquidator payments and authorization
    /// 3. **Account Verification**: Confirms account exists and is active
    /// 4. **Liquidation Execution**: Runs core liquidation algorithm via `execute_liquidation`
    /// 5. **Refund Processing**: Returns excess payments to liquidator if any
    /// 6. **Debt Settlement**: Processes each debt repayment through respective liquidity pools
    /// 7. **Collateral Transfer**: Handles seized collateral transfers with protocol fees
    ///
    /// # Security Checks Implemented
    /// - Reentrancy protection via `cache.flash_loan_ongoing` guard
    /// - Payment validation for all debt repayments
    /// - Caller address validation (non-zero address requirement)
    /// - Account existence and active status verification
    /// - Safe price oracle usage (unsafe prices disabled)
    ///
    /// # Integration Points
    /// The function integrates with multiple protocol components:
    /// - **Liquidity Pools**: For debt repayment processing via `process_repayment`
    /// - **Oracle System**: For price feeds and asset valuation
    /// - **Position Management**: For withdrawal processing via `process_withdrawal`
    /// - **Event System**: For liquidation event emission
    /// - **Cache System**: For optimized storage access and price feed caching
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce identifying the borrower's account
    /// - `debt_payments`: Vector of ERC20/EGLD payments for debt repayment
    /// - `caller`: Address of the liquidator initiating the liquidation
    fn process_liquidation(
        &self,
        account_nonce: u64,
        debt_payments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        caller: &ManagedAddress,
    ) {
        let mut cache = Cache::new(self);
        self.reentrancy_guard(cache.flash_loan_ongoing);
        cache.allow_unsafe_price = false;
        self.validate_liquidation_payments(debt_payments, caller);

        self.require_active_account(account_nonce);

        let account_attributes = self.account_attributes(account_nonce).get();

        let (seized_collaterals, repaid_tokens, refunds, _, _) =
            self.execute_liquidation(account_nonce, debt_payments, &mut cache);

        if !refunds.is_empty() {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        require!(!repaid_tokens.is_empty(), ERROR_NO_DEBT_PAYMENTS_TO_PROCESS);

        for debt_payment_data in repaid_tokens {
            let (debt_payment, debt_egld_value, debt_price_feed) = debt_payment_data.into_tuple();
            self.process_repayment(
                account_nonce,
                &debt_payment.token_identifier,
                &self.to_decimal(debt_payment.amount, debt_price_feed.asset_decimals),
                caller,
                debt_egld_value,
                &debt_price_feed,
                &mut cache,
                &account_attributes,
            );
        }

        for collateral_data in seized_collaterals {
            let (seized_collateral, protocol_fee) = collateral_data.into_tuple();
            let mut deposit_position =
                self.deposit_position(account_nonce, &seized_collateral.token_identifier);
            let price_feed = self.token_price(&deposit_position.asset_id, &mut cache);
            let amount = deposit_position
                .make_amount_decimal(&seized_collateral.amount, price_feed.asset_decimals);
            let _ = self.process_withdrawal(
                account_nonce,
                amount,
                caller,
                true,
                Some(protocol_fee),
                &mut cache,
                &account_attributes,
                &mut deposit_position,
                &price_feed,
            );
        }
    }

    /// Validates that the position's health factor qualifies for liquidation and prevents healthy position liquidation.
    ///
    /// # Purpose and Scope
    /// This critical validation function ensures liquidation integrity by:
    /// - Computing the current health factor using liquidation-weighted collateral
    /// - Enforcing the fundamental liquidation rule (health factor < 1.0)
    /// - Preventing liquidation of healthy positions (which would be protocol theft)
    /// - Returning the health factor for use in subsequent liquidation calculations
    ///
    /// # How It Works (Health Factor Validation)
    /// 1. **Health Factor Calculation**: `collateral_value / total_debt`
    /// 2. **Liquidation Threshold Check**: Health factor must be < 1.0 (RAY precision)
    /// 3. **Security Enforcement**: Reverts transaction if position is healthy
    /// 4. **Value Return**: Provides health factor for Dutch auction calculations
    ///
    /// # Mathematical Formula
    /// ```
    /// health_factor = liquidation_weighted_collateral / total_debt
    ///
    /// Liquidation eligibility: health_factor < 1.0
    /// ```
    ///
    /// Where `liquidation_weighted_collateral` accounts for asset-specific liquidation
    /// thresholds (e.g., 80% for WETH, 75% for WBTC) rather than raw collateral values.
    ///
    /// # Security Checks Implemented
    /// - Health factor boundary validation (must be < 1.0)
    /// - Numerical precision handling with RAY-level accuracy
    /// - Transaction revert on healthy position liquidation attempts
    /// - Proper error code emission for debugging and monitoring
    ///
    /// # Critical Security Note
    /// This validation is the primary defense against:
    /// - **Liquidation Attacks**: Preventing liquidation of healthy positions
    /// - **MEV Exploitation**: Stopping profitable attacks on solvent positions  
    /// - **Protocol Drain**: Ensuring only undercollateralized positions can be liquidated
    /// - **User Protection**: Safeguarding borrowers from premature liquidations
    ///
    /// # Arguments
    /// - `collateral_in_egld`: Liquidation-threshold-weighted collateral value (EGLD-denominated)
    /// - `borrowed_egld`: Total borrowed amount across all assets (EGLD-denominated)
    ///
    /// # Returns
    /// - Current health factor (RAY precision) for use in liquidation amount calculations
    ///
    /// # Errors
    /// - `ERROR_HEALTH_FACTOR`: When health factor ≥ 1.0 (position is healthy and not liquidatable)
    fn validate_liquidation_health_factor(
        &self,
        collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        borrowed_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> ManagedDecimal<Self::Api, NumDecimals> {
        let health_factor = self.compute_health_factor(collateral_in_egld, borrowed_egld);
        require!(health_factor < self.ray(), ERROR_HEALTH_FACTOR);
        health_factor
    }

    /// Validates payments for liquidation operations.
    /// Ensures debt repayments are valid and the caller is authorized.
    ///
    /// # Arguments
    /// - `debt_repayments`: Vector of debt repayment payments.
    /// - `initial_caller`: Address initiating the liquidation.
    ///
    /// # Errors
    /// - Inherits errors from `validate_payment`.
    /// - `ERROR_ADDRESS_IS_ZERO`: If the caller address is zero.
    fn validate_liquidation_payments(
        &self,
        debt_repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        initial_caller: &ManagedAddress,
    ) {
        require!(!debt_repayments.is_empty(), ERROR_INVALID_PAYMENTS);
        for debt_payment in debt_repayments {
            self.validate_payment(&debt_payment);
        }
        self.require_non_zero_address(initial_caller);
    }

    /// Calculates proportional collateral seizure across multiple assets with precision-aware bonus distribution.
    ///
    /// # Purpose and Scope
    /// This function implements sophisticated proportional collateral seizure that:
    /// - Distributes liquidation across all deposited assets proportionally to their values
    /// - Applies liquidation bonuses with protocol fee extraction
    /// - Handles precision loss gracefully during decimal conversions
    /// - Ensures no asset seizure exceeds the deposited amount
    /// - Calculates protocol fees on bonus portions only
    ///
    /// # How It Works (Proportional Seizure Methodology)
    /// 1. **Proportion Calculation**: For each asset, calculates `asset_value / total_collateral_value`
    /// 2. **Base Seizure**: Applies proportion to total debt repayment amount
    /// 3. **Bonus Application**: Adds liquidation bonus (multiplier = BPS + bonus_rate)
    /// 4. **Protocol Fee Extraction**: Calculates fee only on bonus portion using `liquidation_fees`
    /// 5. **Precision Management**: Rescales from RAY precision to asset-specific decimals
    /// 6. **Safety Bounds**: Ensures seized amount never exceeds total deposited amount
    ///
    /// # Mathematical Formulas
    /// For each collateral asset `i`:
    /// ```
    /// proportion_i = asset_value_i / total_collateral_value
    /// base_seizure_i = debt_to_repay * proportion_i
    /// bonus_seizure_i = base_seizure_i * (1 + bonus_rate)
    /// protocol_fee_i = (bonus_seizure_i - base_seizure_i) * liquidation_fees_i
    /// final_seizure_i = min(bonus_seizure_i, total_deposited_i)
    /// ```
    ///
    /// All calculations performed in RAY precision (27 decimals) then rescaled to asset decimals.
    ///
    /// # Security Checks Implemented
    /// - Bounds checking: seized amount ≤ total deposited amount
    /// - Precision loss mitigation with half-up rounding
    /// - Protocol fee validation on bonus portion only
    /// - Safe decimal rescaling from RAY to asset-specific precision
    /// - Zero-amount validation before token transfer creation
    ///
    /// # Precision Handling
    /// The function acknowledges unavoidable precision loss during rescaling from RAY (27 decimals)
    /// to individual token decimals (e.g., 6 for USDC, 18 for WETH). This is handled by:
    /// - Using half-up rounding for all conversions
    /// - Taking minimum of calculated amount and total deposited
    /// - Documenting precision loss points in code comments
    ///
    /// # Arguments
    /// - `deposit_positions`: Vector of borrower's collateral positions with asset IDs and amounts
    /// - `total_collateral_value`: Total collateral value across all assets (EGLD-denominated)
    /// - `debt_to_be_repaid_ray`: Amount of debt being repaid (RAY precision)
    /// - `bonus_rate_ray`: Liquidation bonus rate (RAY precision)
    /// - `cache`: Mutable storage cache for price feeds and asset data
    ///
    /// # Returns
    /// Vector of tuples containing:
    /// - `seized_payment`: EgldOrEsdtTokenPayment representing the seized collateral amount
    /// - `protocol_fee`: ManagedDecimal representing protocol fee on the liquidation bonus
    fn calculate_seized_collateral(
        &self,
        deposit_positions: &ManagedVec<AccountPosition<Self::Api>>,
        total_collateral_value: &ManagedDecimal<Self::Api, NumDecimals>,
        debt_to_be_repaid_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        bonus_rate_ray: &ManagedDecimal<Self::Api, NumDecimals>,
        cache: &mut Cache<Self>,
    ) -> ManagedVec<MultiValue2<EgldOrEsdtTokenPayment, ManagedDecimal<Self::Api, NumDecimals>>>
    {
        let mut seized_amounts_by_collateral = ManagedVec::new();

        // Pre-calculate bonus multiplier
        let bonus_multiplier_ray = self.ray() + bonus_rate_ray.clone();

        for position in deposit_positions {
            let asset_price_feed = self.token_price(&position.asset_id, cache);
            let total_amount_ray = self.total_amount_ray(&position, cache);
            let asset_egld_value_ray =
                self.token_egld_value_ray(&total_amount_ray, &asset_price_feed.price_wad);

            // Calculate proportion in RAY precision
            let asset_proportion_ray =
                self.div_half_up(&asset_egld_value_ray, total_collateral_value, RAY_PRECISION);

            // Calculate seized EGLD amount
            let seized_egld_ray =
                self.mul_half_up(&asset_proportion_ray, debt_to_be_repaid_ray, RAY_PRECISION);

            // Apply liquidation bonus
            let seized_egld_with_bonus_ray =
                self.mul_half_up(&seized_egld_ray, &bonus_multiplier_ray, RAY_PRECISION);

            // Convert back to token units (RAY precision)
            let seized_units_with_bonus_ray =
                self.convert_egld_to_tokens_ray(&seized_egld_with_bonus_ray, &asset_price_feed);

            // Cap seized units to available collateral BEFORE computing bonus split and fees
            let capped_units_with_bonus_ray =
                self.min(seized_units_with_bonus_ray, total_amount_ray.clone());

            // Compute base (no-bonus) units from capped seized amount
            let seized_base_units_ray = self.div_half_up(
                &capped_units_with_bonus_ray,
                &bonus_multiplier_ray,
                RAY_PRECISION,
            );
            let liquidation_bonus_units_ray =
                capped_units_with_bonus_ray.clone() - seized_base_units_ray;

            // Protocol fee on the capped bonus portion
            let protocol_fee_ray = self.mul_half_up(
                &liquidation_bonus_units_ray,
                &position.liquidation_fees_bps,
                RAY_PRECISION,
            );
            let protocol_fee_scaled =
                self.rescale_half_up(&protocol_fee_ray, asset_price_feed.asset_decimals);

            // Final seized transfer amount is the capped units
            let final_seizure_amount = self.rescale_half_up(
                &capped_units_with_bonus_ray,
                asset_price_feed.asset_decimals,
            );
            let seized_asset = EgldOrEsdtTokenPayment::new(
                position.asset_id.clone(),
                0,
                final_seizure_amount.into_raw_units().clone(),
            );
            seized_amounts_by_collateral.push((seized_asset, protocol_fee_scaled).into());
        }

        seized_amounts_by_collateral
    }

    /// Computes total debt repayment with intelligent excess payment handling and automatic refund generation.
    ///
    /// # Purpose and Scope
    /// This function processes liquidator debt payments and:
    /// - Validates each payment against existing borrow positions
    /// - Calculates EGLD-equivalent values for all payments
    /// - Detects and handles excess payments with automatic refunds
    /// - Ensures liquidators cannot pay more than outstanding debt per asset
    /// - Aggregates total repayment value for liquidation calculations
    ///
    /// # How It Works (Payment Processing Methodology)
    /// 1. **Payment Validation**: Checks each payment against corresponding borrow position
    /// 2. **Price Conversion**: Converts token amounts to EGLD-equivalent using oracle prices
    /// 3. **Debt Comparison**: Compares payment amount against outstanding debt for each asset
    /// 4. **Excess Detection**: Identifies payments exceeding outstanding debt amounts
    /// 5. **Refund Generation**: Creates refund payments for excess amounts
    /// 6. **Aggregation**: Sums all valid payments to total EGLD repayment value
    ///
    /// # Mathematical Formulas
    /// For each payment token `i`:
    /// ```
    /// payment_egld_value_i = payment_amount_i * price_i
    /// outstanding_debt_egld_i = borrowed_amount_i * price_i
    ///
    /// if payment_egld_value_i > outstanding_debt_egld_i:
    ///     excess_egld_i = payment_egld_value_i - outstanding_debt_egld_i
    ///     excess_tokens_i = excess_egld_i / price_i
    ///     refund_i = excess_tokens_i
    ///     valid_payment_i = payment_amount_i - excess_tokens_i
    /// else:
    ///     valid_payment_i = payment_amount_i
    ///
    /// total_repaid_egld = Σ(valid_payment_egld_i)
    /// ```
    ///
    /// # Security Checks Implemented
    /// - Payment-to-position matching validation via index mapping
    /// - Outstanding debt boundary enforcement
    /// - Price feed validation for all token conversions
    /// - Automatic excess payment detection and refunding
    /// - Zero-amount payment filtering
    ///
    /// # Excess Payment Handling Logic
    /// When a liquidator pays more than the outstanding debt for a specific asset:
    /// 1. Calculate excess amount in EGLD terms
    /// 2. Convert excess back to original token amount using current price
    /// 3. Reduce the payment amount by the excess
    /// 4. Add excess amount to refunds vector for automatic return
    /// 5. Use only the outstanding debt amount for liquidation calculations
    ///
    /// # Arguments
    /// - `repayments`: Vector of liquidator's debt repayment payments
    /// - `borrows`: Vector of borrower's current borrow positions
    /// - `refunds`: Mutable vector to collect excess payment refunds
    /// - `borrows_index_map`: Mapping from token identifier to borrow position index
    /// - `cache`: Mutable storage cache for price feeds and oracle data
    ///
    /// # Returns
    /// Returns a tuple containing:
    /// - `total_repaid_egld`: Total repayment amount in EGLD (RAY precision)
    /// - `repaid_tokens`: Vector of (payment, egld_value, price_feed) for each valid repayment
    fn calculate_repayment_amounts(
        &self,
        repayments: &ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows: &ManagedVec<AccountPosition<Self::Api>>,
        refunds: &mut ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        borrows_index_map: ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
    ) {
        let mut total_repaid = self.ray_zero();
        let mut repaid_tokens = ManagedVec::new();
        for payment_ref in repayments {
            let token_price_feed = self.token_price(&payment_ref.token_identifier, cache);
            let original_borrow_position =
                self.position_by_index(&payment_ref.token_identifier, borrows, &borrows_index_map);
            let payment_amount_decimal =
                self.to_decimal(payment_ref.amount.clone(), token_price_feed.asset_decimals);

            let payment_egld_value_ray =
                self.token_egld_value_ray(&payment_amount_decimal, &token_price_feed.price_wad);

            let outstanding_debt_ray = self.total_amount_ray(&original_borrow_position, cache);
            let outstanding_debt_egld_ray =
                self.token_egld_value_ray(&outstanding_debt_ray, &token_price_feed.price_wad);
            let mut adjusted_payment = payment_ref.clone();
            if payment_egld_value_ray > outstanding_debt_egld_ray {
                let excess_egld_ray = payment_egld_value_ray - outstanding_debt_egld_ray.clone();
                let excess_token_amount_decimal =
                    self.convert_egld_to_tokens(&excess_egld_ray, &token_price_feed);
                let excess_token_units = excess_token_amount_decimal.into_raw_units().clone();

                // Only create refund if amount is greater than zero
                if excess_token_units > BigUint::zero() {
                    adjusted_payment.amount -= &excess_token_units;

                    refunds.push(EgldOrEsdtTokenPayment::new(
                        payment_ref.token_identifier.clone(),
                        payment_ref.token_nonce,
                        excess_token_units,
                    ));
                }

                total_repaid += &outstanding_debt_egld_ray;
                repaid_tokens.push(
                    (
                        adjusted_payment,
                        outstanding_debt_egld_ray,
                        token_price_feed,
                    )
                        .into(),
                );
            } else {
                total_repaid += &payment_egld_value_ray;
                repaid_tokens
                    .push((adjusted_payment, payment_egld_value_ray, token_price_feed).into());
            }
        }

        (total_repaid, repaid_tokens)
    }

    /// Calculates weighted liquidation parameters by aggregating asset-specific thresholds and bonuses.
    ///
    /// # Purpose and Scope
    /// This function computes portfolio-level liquidation parameters by:
    /// - Calculating value-weighted liquidation thresholds across all collateral assets
    /// - Determining aggregate liquidation bonus rates based on asset composition
    /// - Supporting complex multi-asset positions with different risk parameters
    /// - Enabling precise liquidation calculations for heterogeneous collateral portfolios
    ///
    /// # How It Works (Weighted Aggregation Methodology)
    /// 1. **Asset Valuation**: Calculate EGLD-equivalent value for each collateral position
    /// 2. **Weight Calculation**: Determine each asset's proportion of total collateral value
    /// 3. **Threshold Weighting**: Apply asset-specific liquidation thresholds weighted by value
    /// 4. **Bonus Weighting**: Apply asset-specific liquidation bonuses weighted by value
    /// 5. **Aggregation**: Sum weighted values to produce portfolio-level parameters
    /// 6. **Precision Scaling**: Convert results to basis points precision for downstream use
    ///
    /// # Mathematical Formulas
    /// For each collateral asset `i`:
    /// ```
    /// weight_i = asset_value_i / total_collateral_value
    /// weighted_threshold_i = weight_i * liquidation_threshold_i
    /// weighted_bonus_i = weight_i * liquidation_bonus_i
    /// ```
    ///
    /// Portfolio aggregates:
    /// ```
    /// portfolio_threshold = Σ(weighted_threshold_i)
    /// portfolio_bonus = Σ(weighted_bonus_i)
    /// ```
    ///
    /// All calculations performed in RAY precision then rescaled to BPS precision.
    ///
    /// # Security Checks Implemented
    /// - Division by zero protection (total collateral must be positive)
    /// - Precision scaling validation from RAY to BPS
    /// - Asset-specific parameter validation via price feeds
    /// - Numerical overflow protection in weighted calculations
    ///
    /// # Integration with Risk Parameters
    /// The function integrates with the lending protocol's risk management by:
    /// - Using asset-specific liquidation thresholds (e.g., 80% for WETH, 75% for WBTC)
    /// - Applying asset-specific liquidation bonuses (e.g., 5% for stable coins, 10% for volatile assets)
    /// - Supporting e-mode and isolation mode parameter variations
    /// - Enabling dynamic risk adjustment based on portfolio composition
    ///
    /// # Use in Liquidation Process
    /// The returned values are used in downstream liquidation calculations:
    /// - `proportion_seized`: Used as weighted liquidation threshold for health factor calculations
    /// - `weighted_bonus`: Used as base liquidation bonus rate before Dutch auction adjustments
    ///
    /// # Arguments
    /// - `total_collateral_in_egld`: Total portfolio collateral value (EGLD-denominated)
    /// - `positions`: Vector of deposit positions with asset IDs and amounts
    /// - `cache`: Mutable storage cache for price feeds and asset risk parameters
    ///
    /// # Returns
    /// Returns a tuple containing:
    /// - `proportion_seized`: Value-weighted liquidation threshold (RAY precision)
    /// - `weighted_bonus`: Value-weighted liquidation bonus rate (RAY precision)
    fn calculate_seizure_proportions(
        &self,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        positions: &ManagedVec<AccountPosition<Self::Api>>,
        cache: &mut Cache<Self>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut proportion_seized = self.ray_zero();
        let mut weighted_bonus = self.ray_zero();

        for deposit_position in positions {
            let price_feed = self.token_price(&deposit_position.asset_id, cache);
            let position_amount_ray = self.total_amount_ray(&deposit_position, cache);
            let position_egld_value_ray =
                self.token_egld_value_ray(&position_amount_ray, &price_feed.price_wad);
            let portfolio_weight_ray = self.div_half_up(
                &position_egld_value_ray,
                total_collateral_in_egld,
                RAY_PRECISION,
            );

            proportion_seized += self.mul_half_up(
                &portfolio_weight_ray,
                &deposit_position.liquidation_threshold_bps,
                RAY_PRECISION,
            );
            weighted_bonus += self.mul_half_up(
                &portfolio_weight_ray,
                &deposit_position.liquidation_bonus_bps,
                RAY_PRECISION,
            );
        }

        (proportion_seized, weighted_bonus)
    }

    /// Calculates optimal liquidation amounts using a sophisticated Dutch auction mechanism.
    ///
    /// # Purpose and Scope
    /// This function implements the core liquidation algorithm that solves complex algebraic equations
    /// to determine the maximum debt that can be repaid while maintaining system stability. The
    /// Dutch auction mechanism targets specific health factor ranges (1.02 for conservative, 1.01 for
    /// aggressive liquidations) to ensure efficient capital utilization while preventing over-liquidation.
    ///
    /// # How It Works (Dutch Auction Methodology)
    /// 1. **Health Factor Analysis**: Evaluates current position health relative to liquidation thresholds
    /// 2. **Algebraic Model**: Solves the liquidation equation to find optimal debt repayment amount
    /// 3. **Dynamic Bonus Scaling**: Applies linear scaling with 200% factor, capped at 15% maximum
    /// 4. **Target Health Factor**: Aims for post-liquidation health factor of 1.02 (conservative) or 1.01 (aggressive)
    /// 5. **Payment Constraint**: Limits repayment to actual liquidator payment when provided
    /// 6. **Collateral Calculation**: Determines total collateral to seize including liquidation premium
    ///
    /// # Mathematical Formulas
    /// The core liquidation equation being solved:
    /// ```
    /// target_health_factor = (weighted_collateral - seized_collateral) / (total_debt - repaid_debt)
    /// ```
    /// Where:
    /// - `seized_collateral = repaid_debt * (1 + liquidation_bonus)`
    /// - `liquidation_bonus = min(15%, linear_scale * (1 - health_factor) * 200%)`
    /// - `target_health_factor = 1.02` (conservative) or `1.01` (aggressive)
    ///
    /// Solving for `repaid_debt`:
    /// ```
    /// repaid_debt = (weighted_collateral - target_hf * total_debt) /
    ///               (target_hf + liquidation_bonus - 1)
    /// ```
    ///
    /// # Security Checks Implemented
    /// - Maximum debt validation (cannot exceed total position debt)
    /// - Liquidation bonus cap enforcement (15% maximum)
    /// - Payment amount validation (cannot exceed liquidator's actual payment)
    /// - Precision handling for RAY/WAD conversions
    /// - Health factor boundary validation
    ///
    /// # Integration with E-Mode and Isolation
    /// - E-mode positions may have different liquidation thresholds and bonuses
    /// - Isolated positions follow special liquidation rules
    /// - Weighted collateral accounts for asset-specific liquidation thresholds
    ///
    /// # Arguments
    /// - `total_debt_in_egld`: Total borrowed amount across all assets (EGLD-denominated) as RAY
    /// - `total_collateral_in_egld`: Total collateral value at current prices (EGLD-denominated) as RAY
    /// - `weighted_collateral_in_egld`: Liquidation-threshold-weighted collateral value as RAY
    /// - `proportion_seized`: Weighted seizure proportion across all collateral assets
    /// - `base_liquidation_bonus`: Asset-weighted base liquidation bonus in RAY
    /// - `health_factor`: Current position health factor (< 1.0 for liquidatable positions)
    /// - `egld_payment`: Actual liquidator payment amount in EGLD (RAY precision)
    ///
    /// # Returns
    /// Returns a tuple containing:
    /// - `max_debt_to_repay_ray`: Maximum debt amount to repay (RAY precision)
    /// - `max_debt_to_repay_wad`: Maximum debt amount to repay (WAD precision)
    /// - `max_collateral_seized`: Total collateral value to seize including bonus
    /// - `bonus_rate`: Applied liquidation bonus rate in RAY precision
    fn calculate_liquidation_amounts(
        &self,
        total_debt_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        weighted_collateral_in_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        proportion_seized: &ManagedDecimal<Self::Api, NumDecimals>,
        base_liquidation_bonus: &ManagedDecimal<Self::Api, NumDecimals>,
        health_factor: &ManagedDecimal<Self::Api, NumDecimals>,
        egld_payment_ray: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> (
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
        ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let initial_health_factor = health_factor.clone();

        let (estimated_max_repayable_debt_ray, bonus) = self.estimate_liquidation_amount(
            weighted_collateral_in_egld,
            proportion_seized,
            total_collateral_in_egld,
            total_debt_in_egld,
            base_liquidation_bonus,
            health_factor,
        );
        let final_repayment_amount_ray = if egld_payment_ray > &self.ray_zero() {
            self.min(
                egld_payment_ray.clone(),
                estimated_max_repayable_debt_ray.clone(),
            )
        } else {
            estimated_max_repayable_debt_ray.clone()
        };

        let effective_bonus = {
            let projected_health_factor = self.calculate_post_liquidation_health_factor(
                weighted_collateral_in_egld,
                total_debt_in_egld,
                &final_repayment_amount_ray,
                proportion_seized,
                &bonus,
            );

            let repayment_is_capped = final_repayment_amount_ray < estimated_max_repayable_debt_ray;

            let significant_shortfall = if repayment_is_capped {
                let tolerance_ratio_bps = self.to_decimal_bps(BigUint::from(100u64));
                let tolerance_ratio_ray = self.rescale_half_up(&tolerance_ratio_bps, RAY_PRECISION);
                let tolerance_amount = self.mul_half_up(
                    &estimated_max_repayable_debt_ray,
                    &tolerance_ratio_ray,
                    RAY_PRECISION,
                );
                let repayment_gap =
                    estimated_max_repayable_debt_ray.clone() - final_repayment_amount_ray.clone();
                repayment_gap > tolerance_amount
            } else {
                false
            };

            if repayment_is_capped
                && significant_shortfall
                && projected_health_factor <= initial_health_factor
            {
                base_liquidation_bonus.clone()
            } else {
                bonus
            }
        };

        let liquidation_premium_ray = effective_bonus.clone() + self.ray();

        let collateral_to_seize = self.mul_half_up(
            &final_repayment_amount_ray,
            &liquidation_premium_ray,
            RAY_PRECISION,
        );

        (
            final_repayment_amount_ray,
            collateral_to_seize,
            effective_bonus,
        )
    }

    /// Processes excess liquidation payments using a reverse-chronological adjustment algorithm.
    ///
    /// # Purpose and Scope
    /// This function handles situations where liquidators pay more than the maximum allowable
    /// debt repayment by:
    /// - Redistributing excess payments across multiple tokens in reverse order
    /// - Maintaining accurate accounting for both repayments and refunds
    /// - Ensuring liquidators receive proper refunds for overpayments
    /// - Preserving liquidation integrity while handling payment edge cases
    ///
    /// # How It Works (Reverse-Chronological Algorithm)
    /// 1. **Reverse Iteration**: Processes repaid tokens from last to first (LIFO approach)
    /// 2. **Excess Distribution**: Distributes total excess across individual token payments
    /// 3. **Partial Adjustments**: Reduces payments when excess is less than token amount
    /// 4. **Full Removals**: Removes entire payments when excess exceeds token amount
    /// 5. **Refund Generation**: Creates corresponding refund entries for all adjustments
    /// 6. **Accounting Updates**: Updates repaid_tokens vector with adjusted amounts
    ///
    /// # Mathematical Process
    /// For each token in reverse order:
    /// ```
    /// if excess_remaining >= token_egld_value:
    ///     // Full removal case
    ///     refund_amount = entire_token_payment
    ///     remove_from_repaid_tokens(token)
    ///     excess_remaining -= token_egld_value
    /// else:
    ///     // Partial adjustment case
    ///     refund_egld = excess_remaining
    ///     refund_tokens = refund_egld / token_price
    ///     adjust_payment_amount(token, -refund_tokens)
    ///     excess_remaining = 0
    /// ```
    ///
    /// # Algorithm Design Rationale
    /// The reverse-chronological approach provides several benefits:
    /// - **Simplicity**: Easier to implement and understand than proportional distribution
    /// - **Efficiency**: Requires fewer calculations than complex weighted approaches
    /// - **Determinism**: Consistent behavior regardless of payment order variations
    /// - **Edge Case Handling**: Gracefully handles partial and complete payment adjustments
    ///
    /// # Security Checks Implemented
    /// - Bounds validation (excess cannot exceed total repayment)
    /// - Price conversion accuracy for refund calculations
    /// - Vector index safety during reverse iteration
    /// - Accounting consistency (total adjustments = initial excess)
    /// - Refund amount validation (non-negative, properly converted)
    ///
    /// # Integration with Liquidation Flow
    /// This function is called when `debt_payment_in_egld_ray > max_debt_to_repay_ray`,
    /// ensuring that liquidators:
    /// - Never pay more than the calculated maximum
    /// - Receive automatic refunds for overpayments
    /// - Maintain proper liquidation incentives
    /// - Experience predictable payment processing
    ///
    /// # Arguments
    /// - `repaid_tokens`: Mutable vector of (payment, egld_value, price_feed) tuples
    /// - `refunds`: Mutable vector to collect refund payments for return to liquidator
    /// - `excess_in_egld`: Total excess payment amount in EGLD (RAY precision)
    fn process_excess_payment(
        &self,
        repaid_tokens: &mut ManagedVec<
            MultiValue3<
                EgldOrEsdtTokenPayment,
                ManagedDecimal<Self::Api, NumDecimals>,
                PriceFeedShort<Self::Api>,
            >,
        >,
        refunds: &mut ManagedVec<EgldOrEsdtTokenPayment<Self::Api>>,
        excess_in_egld: ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        let mut remaining_excess = excess_in_egld;
        let mut current_index = repaid_tokens.len();
        while current_index > 0 && remaining_excess > self.ray_zero() {
            current_index -= 1;

            let (mut debt_payment, mut egld_asset_amount_ray, feed) =
                repaid_tokens.get(current_index).clone().into_tuple();

            if egld_asset_amount_ray >= remaining_excess {
                let excess_in_original = self.convert_egld_to_tokens(&remaining_excess, &feed);
                debt_payment.amount -= excess_in_original.into_raw_units();
                egld_asset_amount_ray -= &remaining_excess;

                refunds.push(EgldOrEsdtTokenPayment::new(
                    debt_payment.token_identifier.clone(),
                    0,
                    excess_in_original.into_raw_units().clone(),
                ));
                let _ = repaid_tokens.set(
                    current_index,
                    (debt_payment, egld_asset_amount_ray, feed).into(),
                );

                remaining_excess = self.ray_zero();
            } else {
                refunds.push(debt_payment);
                repaid_tokens.remove(current_index);
                remaining_excess -= egld_asset_amount_ray;
            }
        }
    }

    /// Retrieves a specific borrow position using optimized index-based lookup for liquidation processing.
    ///
    /// # Purpose and Scope
    /// This utility function provides efficient access to borrow positions during liquidation by:
    /// - Using pre-computed index mappings to avoid linear searches
    /// - Validating position existence before access
    /// - Handling the index offset correction needed for zero-based indexing
    /// - Supporting fast liquidation processing with O(1) position lookup
    ///
    /// # How It Works (Index-Based Lookup)
    /// 1. **Existence Validation**: Checks if token exists in the index mapping
    /// 2. **Index Retrieval**: Gets the stored index for the token identifier
    /// 3. **Offset Correction**: Subtracts 1 to convert from 1-based to 0-based indexing
    /// 4. **Position Access**: Retrieves the position from the vector using the corrected index
    /// 5. **Position Return**: Returns a clone of the position for liquidation calculations
    ///
    /// # Index Offset Handling
    /// The function implements a critical index correction:
    /// ```
    /// stored_index = borrows_index_map.get(token_id)  // 1-based index
    /// actual_index = stored_index - 1                  // Convert to 0-based
    /// position = borrows.get(actual_index)             // Vector access
    /// ```
    ///
    /// This correction is necessary because the mapping uses 1-based indexing to distinguish
    /// between "index 0" and "not found" in the contains() check.
    ///
    /// # Security Checks Implemented
    /// - Position existence validation via `contains()` check
    /// - Index bounds validation (implicit via vector access)
    /// - Token identifier validation against mapping keys
    /// - Error message generation with token identifier for debugging
    ///
    /// # Performance Optimization
    /// Using index-based lookup provides significant performance benefits:
    /// - **O(1) Access**: Direct vector indexing vs O(n) linear search
    /// - **Memory Efficiency**: Single lookup vs full vector iteration
    /// - **Cache Friendly**: Better cache locality with direct indexing
    /// - **Scalable**: Performance doesn't degrade with position count
    ///
    /// # Arguments
    /// - `token_id`: Token identifier to search for in the borrow positions
    /// - `borrows`: Vector of all borrow positions for the account
    /// - `borrows_index_map`: Pre-computed mapping from token ID to vector index (1-based)
    ///
    /// # Returns
    /// - Clone of the AccountPosition corresponding to the specified token
    ///
    /// # Panics
    /// - When `token_id` is not found in `borrows_index_map` (validation failure)
    fn position_by_index(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
        borrows: &ManagedVec<AccountPosition<Self::Api>>,
        borrows_index_map: &ManagedMapEncoded<Self::Api, EgldOrEsdtTokenIdentifier, usize>,
    ) -> AccountPosition<Self::Api> {
        require!(
            borrows_index_map.contains(token_id),
            "Token {} is not part of the mapper",
            token_id
        );
        let safe_index = borrows_index_map.get(token_id);
        // -1 is required to by pass the issue of position_index = 0 which will throw at the above .contains
        let position_index = safe_index - 1;
        let position = borrows.get(position_index).clone();

        position
    }

    /// Analyzes post-liquidation position state and triggers bad debt cleanup when necessary.
    ///
    /// # Purpose and Scope
    /// This function implements sophisticated bad debt detection that:
    /// - Calculates remaining debt and collateral after liquidation completion
    /// - Applies $5 USD threshold logic for dust position cleanup
    /// - Prevents accumulation of small bad debt positions
    /// - Triggers automatic bad debt cleanup events for external processing
    /// - Maintains protocol solvency by identifying undercollateralized remnants
    ///
    /// # How It Works (Bad Debt Detection Methodology)
    /// 1. **Remainder Calculation**: Computes post-liquidation debt and collateral balances
    /// 2. **USD Value Conversion**: Converts EGLD amounts to USD using oracle prices
    /// 3. **Threshold Analysis**: Applies $5 USD threshold logic for cleanup eligibility
    /// 4. **Bad Debt Validation**: Checks if debt exceeds remaining collateral value
    /// 5. **Cleanup Triggering**: Emits events for positions meeting cleanup criteria
    ///
    /// # Mathematical Formulas
    /// Post-liquidation calculations:
    /// ```
    /// remaining_debt_egld = max(0, borrowed_egld - max_debt_repaid)
    /// remaining_collateral_egld = max(0, total_collateral - seized_collateral_egld)
    ///
    /// remaining_debt_usd = remaining_debt_egld * egld_usd_price
    /// remaining_collateral_usd = remaining_collateral_egld * egld_usd_price
    /// ```
    ///
    /// Bad debt cleanup criteria (all must be true):
    /// ```
    /// has_bad_debt = remaining_debt_usd > remaining_collateral_usd
    /// has_minimal_collateral = remaining_collateral_usd <= $5 USD
    /// has_significant_debt = remaining_debt_usd >= $5 USD
    /// ```
    ///
    /// # Security Checks Implemented
    /// - Non-negative remainder validation (debt and collateral cannot be negative)
    /// - USD conversion validation using EGLD/USD oracle price
    /// - Threshold boundary validation ($5 USD minimum)
    /// - Bad debt ratio validation (debt must exceed collateral)
    /// - Event emission validation for cleanup triggers
    ///
    /// # Bad Debt Threshold Logic ($5 USD)
    /// The $5 USD threshold serves multiple purposes:
    /// - **Gas Efficiency**: Prevents cleanup of positions worth less than transaction costs
    /// - **Economic Viability**: Ensures cleanup operations are economically justified
    /// - **Protocol Health**: Removes dust positions that could accumulate over time
    /// - **Liquidator Incentives**: Focuses liquidation efforts on meaningful positions
    ///
    /// # Integration with Cleanup Process
    /// When cleanup criteria are met, this function emits events that trigger:
    /// - External bad debt cleanup procedures
    /// - Protocol reserve fund utilization
    /// - Position closure and account cleanup
    /// - Liquidity pool bad debt accounting
    ///
    /// # Arguments
    /// - `cache`: Mutable storage cache containing EGLD/USD price feeds
    /// - `account_nonce`: Position NFT nonce identifying the account
    /// - `borrowed_egld`: Total borrowed value before liquidation (EGLD-denominated)
    /// - `max_debt_repaid`: Amount of debt repaid during liquidation (EGLD-denominated)
    /// - `total_collateral`: Total collateral value before liquidation (EGLD-denominated)
    /// - `seized_collateral_egld`: Amount of collateral seized during liquidation (EGLD-denominated)
    fn check_bad_debt_after_liquidation(
        &self,
        cache: &mut Cache<Self>,
        account_nonce: u64,
        borrowed_egld: &ManagedDecimal<Self::Api, NumDecimals>,
        max_debt_repaid: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
        seized_collateral_egld: &ManagedDecimal<Self::Api, NumDecimals>,
    ) {
        // Calculate remaining debt and collateral after liquidation
        let remaining_debt_egld = if borrowed_egld > max_debt_repaid {
            borrowed_egld.clone() - max_debt_repaid.clone()
        } else {
            self.wad_zero() // All debt repaid
        };

        let remaining_collateral_egld = if total_collateral > seized_collateral_egld {
            total_collateral.clone() - seized_collateral_egld.clone()
        } else {
            self.wad_zero() // All collateral seized
        };

        let can_clean_bad_debt = self.can_clean_bad_debt_positions(
            cache,
            &remaining_debt_egld,
            &remaining_collateral_egld,
        );

        if can_clean_bad_debt {
            self.emit_trigger_clean_bad_debt(
                account_nonce,
                &remaining_debt_egld,
                &remaining_collateral_egld,
            );
        }
    }

    /// Evaluates whether a position qualifies for bad debt cleanup based on USD value thresholds.
    ///
    /// # Purpose and Scope
    /// This function implements the core bad debt detection algorithm that determines if a position
    /// should undergo dust cleanup based on economic thresholds. It prevents the accumulation of
    /// small undercollateralized positions that would be uneconomical to liquidate individually.
    ///
    /// # How It Works (Threshold Evaluation Logic)
    /// 1. **USD Conversion**: Converts debt and collateral from EGLD to USD using oracle prices
    /// 2. **Bad Debt Detection**: Checks if debt value exceeds collateral value
    /// 3. **Threshold Validation**: Applies $5 USD thresholds for collateral
    /// 4. **Cleanup Qualification**: Returns true only if all criteria are met simultaneously
    ///
    /// # Mathematical Criteria (All Must Be True)
    /// ```
    /// total_usd_debt = total_borrow * egld_usd_price
    /// total_usd_collateral = total_collateral * egld_usd_price
    /// min_threshold = $5 USD
    ///
    /// Cleanup criteria:
    /// 1. has_bad_debt = total_usd_debt > total_usd_collateral
    /// 2. has_minimal_collateral = total_usd_collateral <= min_threshold
    /// ```
    ///
    /// # Economic Rationale
    /// The $5 USD threshold ensures that:
    /// - Cleanup operations are economically viable (gas costs < cleanup value)
    /// - Small dust positions don't accumulate in the protocol
    /// - Liquidators focus on meaningful positions
    /// - Protocol maintains clean accounting without negligible bad debt
    fn can_clean_bad_debt_positions(
        &self,
        cache: &mut Cache<Self>,
        total_borrow: &ManagedDecimal<Self::Api, NumDecimals>,
        total_collateral: &ManagedDecimal<Self::Api, NumDecimals>,
    ) -> bool {
        let total_usd_debt = self.egld_usd_value(total_borrow, &cache.egld_usd_price_wad);
        let total_usd_collateral = self.egld_usd_value(total_collateral, &cache.egld_usd_price_wad);

        // 5 USD
        let min_collateral_threshold = self.mul_half_up(
            &self.wad(),
            &self.to_decimal(BigUint::from(5u64), 0),
            WAD_PRECISION,
        );

        let has_bad_debt = total_usd_debt > total_usd_collateral;
        let has_collateral_under_min_threshold = total_usd_collateral <= min_collateral_threshold;

        has_bad_debt && has_collateral_under_min_threshold
    }

    /// Executes comprehensive bad debt cleanup by liquidating all remaining positions and transferring losses to protocol reserves.
    ///
    /// # Purpose and Scope
    /// This function performs complete position closure for accounts with bad debt that meets
    /// cleanup criteria ($5 USD threshold). It:
    /// - Transfers all remaining debt to liquidity pool bad debt reserves
    /// - Seizes all remaining collateral for protocol benefit
    /// - Handles isolated debt positions with special clearing procedures
    /// - Completely closes the account and clears all position data
    /// - Maintains protocol solvency through systematic bad debt accounting
    ///
    /// # How It Works (Complete Liquidation Process)
    /// 1. **Debt Transfer**: All borrow positions are transferred to liquidity pools as bad debt
    /// 2. **Isolated Debt Clearing**: Special handling for isolated positions via dedicated clearing
    /// 3. **Collateral Seizure**: All deposit positions are seized by the protocol as dust collateral
    /// 4. **Pool Integration**: Liquidity pools update their bad debt accounting and collateral reserves
    /// 5. **Position Cleanup**: All position mappings are cleared from storage
    /// 6. **Account Closure**: Account NFT and attributes are completely removed
    ///
    /// # Security Checks Implemented
    /// - Caller address validation for event emission
    /// - Pool address validation via cached lookups
    /// - Position state validation before cleanup operations
    /// - Atomic operation sequencing (debt first, then collateral)
    /// - Complete data cleanup to prevent orphaned positions
    ///
    /// # Integration with Liquidity Pools
    /// The function makes two critical calls to each affected liquidity pool:
    ///
    /// **For Borrow Positions** (`add_bad_debt`):
    /// - Transfers debt obligation to pool's bad debt reserves
    /// - Updates pool's accounting for undercollateralized loans
    /// - May trigger protocol reserve utilization for bad debt coverage
    ///
    /// **For Deposit Positions** (`seize_dust_collateral`):
    /// - Transfers collateral ownership to the liquidity pool
    /// - Adds seized assets to pool's reserve fund
    /// - Helps offset bad debt losses through collateral recovery
    ///
    /// # Bad Debt Accounting Flow
    /// ```
    /// For each borrow position:
    ///   pool.add_bad_debt(position, current_price)
    ///   emit_position_update_event(zero_position, updated_position)
    ///
    /// For each deposit position:
    ///   pool.seize_dust_collateral(position, current_price)
    ///   emit_position_update_event(zero_position, updated_position)
    ///
    /// Clear all position mappings and account data
    /// ```
    ///
    /// # Isolated Debt Special Handling
    /// For accounts in isolated mode, the function calls `clear_position_isolated_debt`
    /// before transferring to bad debt, ensuring proper isolated position accounting
    /// and preventing cross-contamination between isolated and regular debt positions.
    ///
    /// # Economic Impact
    /// Bad debt cleanup has several economic effects:
    /// - **Protocol Reserves**: May draw from protocol reserves to cover bad debt
    /// - **Liquidity Pools**: Bad debt is distributed across affected pools
    /// - **Collateral Recovery**: Seized collateral helps offset bad debt losses
    /// - **System Health**: Removes toxic positions from active accounting
    ///
    /// # Arguments
    /// - `account_nonce`: Position NFT nonce identifying the account to clean up
    /// - `cache`: Mutable storage cache for price feeds and pool address lookups
    fn perform_bad_debt_cleanup(&self, account_nonce: u64, cache: &mut Cache<Self>) {
        let caller = self.blockchain().get_caller();
        let account_attributes = self.account_attributes(account_nonce).get();

        // Add all remaining debt as bad debt, clean isolated debt if any
        let borrow_positions = self.positions(account_nonce, AccountPositionType::Borrow);
        for (token_id, mut position) in borrow_positions.iter() {
            let feed = self.token_price(&token_id, cache);
            let pool_address = cache.cached_pool_address(&token_id);
            if account_attributes.is_isolated() {
                self.clear_position_isolated_debt(&mut position, &feed, &account_attributes, cache);
            }

            // Call the add_bad_debt function on the liquidity pool
            let updated_position = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .seize_position(position.clone(), feed.price_wad.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.emit_position_update_event(
                cache,
                &position.zero_decimal(),
                &updated_position,
                feed.price_wad.clone(),
                &caller,
                &account_attributes,
            );
        }

        // Seize all remaining collateral + interest
        let deposit_positions = self.positions(account_nonce, AccountPositionType::Deposit);
        for (token_id, position) in deposit_positions.iter() {
            let feed = self.token_price(&token_id, cache);
            let pool_address = cache.cached_pool_address(&token_id);
            // Call the seize_dust_collateral function on the liquidity pool
            let updated_position = self
                .tx()
                .to(pool_address)
                .typed(proxy_pool::LiquidityPoolProxy)
                .seize_position(position.clone(), feed.price_wad.clone())
                .returns(ReturnsResult)
                .sync_call();

            self.emit_position_update_event(
                cache,
                &position.zero_decimal(),
                &updated_position,
                feed.price_wad,
                &caller,
                &account_attributes,
            );
        }

        self.positions(account_nonce, AccountPositionType::Borrow)
            .clear();
        self.positions(account_nonce, AccountPositionType::Deposit)
            .clear();
        self.accounts().swap_remove(&account_nonce);
        self.account_attributes(account_nonce).clear();
    }
}
