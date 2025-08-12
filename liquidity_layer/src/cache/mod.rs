use common_constants::RAY_PRECISION;
use common_structs::MarketParams;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();
/// A snapshot of the pool's state, cached from on-chain storage for efficient access and updates.
///
/// **Scope**: Represents the current market state, including assets, indices, and timestamps.
///
/// **Goal**: Facilitate calculations (e.g., interest rates, utilization) by providing a mutable in-memory view of the pool.
///
/// **Fields**:
/// - All monetary values (`supplied`, `borrowed`, `revenue`) are in `ManagedDecimal` with pool-specific asset_decimals.
/// - Indices (`borrow_index`, `supply_index`) use RAY precision for interest accrual tracking.
/// - Timestamps (`timestamp`, `last_timestamp`) are in milliseconds since the Unix epoch.
pub struct Cache<'a, C>
where
    C: crate::storage::Storage + common_rates::InterestRates,
{
    sc_ref: &'a C,
    /// The amount of the asset supplied by lenders.
    pub supplied_ray: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset currently borrowed.
    pub borrowed_ray: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved for protocol revenue (subset of reserves).
    pub revenue_ray: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the current block (milliseconds since Unix epoch).
    pub timestamp: u64,
    /// The configuration parameters of the pool (e.g., interest rate slopes).
    pub parameters: MarketParams<C::Api>,
    /// The borrow index tracking compounded interest for borrowers.
    pub borrow_index_ray: ManagedDecimal<C::Api, NumDecimals>,
    /// The supply index tracking accrued rewards for suppliers.
    pub supply_index_ray: ManagedDecimal<C::Api, NumDecimals>,
    /// Zero value with pool-specific asset_decimals for comparisons.
    pub zero: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the last state update (milliseconds since Unix epoch).
    pub last_timestamp: u64,
}

impl<'a, C> Cache<'a, C>
where
    C: crate::storage::Storage + common_math::SharedMathModule + common_rates::InterestRates,
{
    /// Constructs a new Cache by reading the current state from on-chain storage.
    ///
    /// **Scope**: Initializes a `Cache` instance with the latest pool data.
    ///
    /// **Goal**: Provide a consistent starting point for calculations and updates.
    ///
    /// # Arguments
    /// - `sc_ref`: Reference to the contract implementing `Storage` and `SharedMathModule`.
    ///
    /// # Returns
    /// - `Cache<Self>`: A new instance containing the cached market state.
    ///
    /// **Security Tip**: Assumes storage getters (`supplied()`, etc.) return valid data; no additional validation here.
    pub fn new(sc_ref: &'a C) -> Self {
        let parameters = sc_ref.parameters().get();
        let timestamp = sc_ref.blockchain().get_block_timestamp_ms();
        Cache {
            zero: sc_ref.to_decimal(BigUint::zero(), parameters.asset_decimals),
            supplied_ray: sc_ref.supplied().get(),
            borrowed_ray: sc_ref.borrowed().get(),
            revenue_ray: sc_ref.revenue().get(),
            timestamp,
            parameters,
            borrow_index_ray: sc_ref.borrow_index().get(),
            supply_index_ray: sc_ref.supply_index().get(),
            last_timestamp: sc_ref.last_timestamp().get(),
            sc_ref,
        }
    }
}

impl<C> Drop for Cache<'_, C>
where
    C: crate::storage::Storage + common_rates::InterestRates,
{
    /// Commits changes to mutable fields back to on-chain storage when the Cache is dropped.
    ///
    /// **Scope**: Ensures the pool's state is persisted after modifications.
    ///
    /// **Goal**: Maintain consistency between in-memory cache and blockchain storage.
    ///
    /// **Fields Updated**: `supplied`, `borrowed`, `revenue`, `borrow_index`, `supply_index`, `last_timestamp`.
    ///
    /// **Security Tip**: Assumes setters (`set()`) handle serialization correctly; no validation here.
    fn drop(&mut self) {
        // commit changes to storage for the mutable fields
        self.sc_ref.supplied().set(&self.supplied_ray);
        self.sc_ref.borrowed().set(&self.borrowed_ray);
        self.sc_ref.revenue().set(&self.revenue_ray);
        self.sc_ref.borrow_index().set(&self.borrow_index_ray);
        self.sc_ref.supply_index().set(&self.supply_index_ray);
        self.sc_ref.last_timestamp().set(self.last_timestamp);
    }
}

impl<C> Cache<'_, C>
where
    C: crate::storage::Storage + common_math::SharedMathModule + common_rates::InterestRates,
{
    /// Converts a raw BigUint value into a ManagedDecimal using the pool's decimal precision.
    ///
    /// **Scope**: Utility to standardize raw values into decimal form for calculations.
    ///
    /// **Goal**: Ensure consistency in decimal handling across pool operations.
    ///
    /// # Arguments
    /// - `value`: The raw `BigUint` value from storage or input.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The value adjusted to pool asset_decimals.
    ///
    /// **Security Tip**: No overflow checks; assumes `value` fits within `BigUint` constraints.
    pub fn decimal_value(&self, value: &BigUint<C::Api>) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref
            .to_decimal(value.clone(), self.parameters.asset_decimals)
    }

    /// Calculates the utilization ratio of the pool (borrowed / supplied).
    ///
    /// **Scope**: Measures how much of the supplied assets are currently borrowed.
    ///
    /// **Goal**: Provide a key metric for interest rate calculations.
    ///
    /// **Formula**:
    /// - If `supplied == 0`: Returns 0 (RAY-based).
    /// - Otherwise: `borrowed / supplied`.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: Utilization ratio (RAY-based).
    ///
    /// **Security Tip**: Handles division-by-zero by returning 0 when `supplied` is zero.
    pub fn calculate_utilization(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        if self.supplied_ray == self.sc_ref.ray_zero() {
            self.sc_ref.ray_zero()
        } else {
            let total_borrowed = self.calculate_original_borrow_ray(&self.borrowed_ray);
            let total_supplied = self.calculate_original_supply_ray(&self.supplied_ray);
            self.sc_ref
                .div_half_up(&total_borrowed, &total_supplied, RAY_PRECISION)
        }
    }

    /// Calculates the effective reserves available (reserves minus protocol revenue).
    ///
    /// **Scope**: Determines the usable reserve amount after accounting for protocol fees.
    ///
    /// **Goal**: Ensure accurate reserve availability for withdrawals or loans.
    ///
    /// **Formula**:
    /// - If `reserves >= revenue`: `reserves - revenue`.
    /// - Otherwise: 0.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: Available reserves in pool asset_decimals.
    ///
    /// **Security Tip**: Prevents underflow by returning 0 if `revenue` exceeds `reserves`.
    pub fn calculate_reserves(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        let current_pool_balance = self
            .sc_ref
            .blockchain()
            .get_sc_balance(&self.parameters.asset_id, 0);
        self.decimal_value(&current_pool_balance)
    }

    /// Checks if the pool has sufficient effective reserves for a given amount.
    ///
    /// **Scope**: Validates reserve availability for operations like withdrawals.
    ///
    /// **Goal**: Prevent overdrawing reserves beyond what's available.
    ///
    /// # Arguments
    /// - `amount`: The amount to check against (`ManagedDecimal`).
    ///
    /// # Returns
    /// - `bool`: True if `get_reserves() >= amount`, false otherwise.
    pub fn has_reserves(&self, amount: &ManagedDecimal<C::Api, NumDecimals>) -> bool {
        self.calculate_reserves() >= *amount
    }

    /// Checks if the given asset matches the pool's asset.
    ///
    /// **Scope**: Validates asset compatibility for pool operations.
    ///
    /// **Goal**: Prevent mismatches in asset types during deposits, borrows, etc.
    ///
    /// # Arguments
    /// - `asset`: The asset identifier to compare (`EgldOrEsdtTokenIdentifier`).
    ///
    /// # Returns
    /// - `bool`: True if `pool_asset == asset`, false otherwise.
    pub fn is_same_asset(&self, asset: &EgldOrEsdtTokenIdentifier<C::Api>) -> bool {
        self.parameters.asset_id == *asset
    }

    pub fn calculate_scaled_supply(
        &self,
        amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref
            .div_half_up(amount, &self.supply_index_ray, RAY_PRECISION)
    }

    pub fn calculate_scaled_borrow(
        &self,
        amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref
            .div_half_up(amount, &self.borrow_index_ray, RAY_PRECISION)
    }

    pub fn calculate_original_supply(
        &self,
        scaled_amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref.scaled_to_original(
            scaled_amount,
            &self.supply_index_ray,
            self.parameters.asset_decimals,
        )
    }

    pub fn calculate_original_supply_ray(
        &self,
        scaled_amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref
            .scaled_to_original_ray(scaled_amount, &self.supply_index_ray)
    }

    pub fn calculate_original_borrow(
        &self,
        scaled_amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref.scaled_to_original(
            scaled_amount,
            &self.borrow_index_ray,
            self.parameters.asset_decimals,
        )
    }

    pub fn calculate_original_borrow_ray(
        &self,
        scaled_amount: &ManagedDecimal<C::Api, NumDecimals>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        self.sc_ref
            .scaled_to_original_ray(scaled_amount, &self.borrow_index_ray)
    }
}
