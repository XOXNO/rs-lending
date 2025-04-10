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
/// - All monetary values (`supplied`, `reserves`, `borrowed`, `revenue`) are in `ManagedDecimal` with pool-specific asset_decimals.
/// - Indices (`borrow_index`, `supply_index`) use RAY precision for interest accrual tracking.
/// - Timestamps (`timestamp`, `last_timestamp`) are in seconds since the Unix epoch.
pub struct Cache<'a, C>
where
    C: crate::storage::Storage,
{
    sc_ref: &'a C,
    /// The amount of the asset supplied by lenders.
    pub supplied: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset held as reserves (includes protocol revenue).
    pub reserves: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset currently borrowed.
    pub borrowed: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset pending to be collected as bad debt.
    pub bad_debt: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved for protocol revenue (subset of reserves).
    pub revenue: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the current block (seconds since Unix epoch).
    pub timestamp: u64,
    /// The asset identifier of the pool (EGLD or ESDT token).
    pub pool_asset: EgldOrEsdtTokenIdentifier<C::Api>,
    /// The configuration parameters of the pool (e.g., interest rate slopes).
    pub params: MarketParams<C::Api>,
    /// The borrow index tracking compounded interest for borrowers.
    pub borrow_index: ManagedDecimal<C::Api, NumDecimals>,
    /// The supply index tracking accrued rewards for suppliers.
    pub supply_index: ManagedDecimal<C::Api, NumDecimals>,
    /// Zero value with pool-specific asset_decimals for comparisons.
    pub zero: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the last state update (seconds since Unix epoch).
    pub last_timestamp: u64,
}

impl<'a, C> Cache<'a, C>
where
    C: crate::storage::Storage + common_math::SharedMathModule,
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
        let params = sc_ref.params().get();
        Cache {
            zero: ManagedDecimal::from_raw_units(BigUint::zero(), params.asset_decimals),
            supplied: sc_ref.supplied().get(),
            reserves: sc_ref.reserves().get(),
            borrowed: sc_ref.borrowed().get(),
            bad_debt: sc_ref.bad_debt().get(),
            revenue: sc_ref.revenue().get(),
            timestamp: sc_ref.blockchain().get_block_timestamp(),
            pool_asset: sc_ref.pool_asset().get(),
            params: params,
            borrow_index: sc_ref.borrow_index().get(),
            supply_index: sc_ref.supply_index().get(),
            last_timestamp: sc_ref.last_timestamp().get(),
            sc_ref,
        }
    }
}

impl<C> Drop for Cache<'_, C>
where
    C: crate::storage::Storage,
{
    /// Commits changes to mutable fields back to on-chain storage when the Cache is dropped.
    ///
    /// **Scope**: Ensures the pool’s state is persisted after modifications.
    ///
    /// **Goal**: Maintain consistency between in-memory cache and blockchain storage.
    ///
    /// **Fields Updated**: `supplied`, `reserves`, `borrowed`, `revenue`, `borrow_index`, `supply_index`, `last_timestamp`.
    ///
    /// **Security Tip**: Assumes setters (`set()`) handle serialization correctly; no validation here.
    fn drop(&mut self) {
        // commit changes to storage for the mutable fields
        self.sc_ref.supplied().set(&self.supplied);
        self.sc_ref.reserves().set(&self.reserves);
        self.sc_ref.borrowed().set(&self.borrowed);
        self.sc_ref.bad_debt().set(&self.bad_debt);
        self.sc_ref.revenue().set(&self.revenue);
        self.sc_ref.borrow_index().set(&self.borrow_index);
        self.sc_ref.supply_index().set(&self.supply_index);
        self.sc_ref.last_timestamp().set(&self.last_timestamp);
    }
}

impl<'a, C> Cache<'a, C>
where
    C: crate::storage::Storage + common_math::SharedMathModule,
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
    pub fn get_decimal_value(
        &self,
        value: &BigUint<C::Api>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        ManagedDecimal::from_raw_units(value.clone(), self.params.asset_decimals)
    }

    /// Computes the utilization ratio of the pool (borrowed / supplied).
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
    pub fn get_utilization(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        if self.supplied == self.zero {
            self.sc_ref.to_decimal_ray(BigUint::zero())
        } else {
            let utilization_ratio = self.sc_ref.div_half_up(
                &self.borrowed,
                &self.supplied,
                common_constants::RAY_PRECISION,
            );

            utilization_ratio
        }
    }

    /// Computes the total capital of the pool (reserves + borrowed).
    ///
    /// **Scope**: Calculates the total assets either reserved or lent out.
    ///
    /// **Goal**: Provide insight into the pool’s active capital for auditing and analytics.
    ///
    /// **Formula**:
    /// - `total_capital = reserves + borrowed`.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: Total capital in pool asset_decimals.
    ///
    pub fn get_total_capital(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        let reserve_amount = self.reserves.clone();
        let borrowed = self.borrowed.clone();

        reserve_amount + borrowed
    }

    /// Computes the effective reserves available (reserves minus protocol revenue).
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
    pub fn get_reserves(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        if self.reserves >= self.revenue {
            self.reserves.clone() - self.revenue.clone()
        } else {
            ManagedDecimal::from_raw_units(BigUint::zero(), self.params.asset_decimals)
        }
    }

    /// Checks if the pool has sufficient effective reserves for a given amount.
    ///
    /// **Scope**: Validates reserve availability for operations like withdrawals.
    ///
    /// **Goal**: Prevent overdrawing reserves beyond what’s available.
    ///
    /// # Arguments
    /// - `amount`: The amount to check against (`ManagedDecimal`).
    ///
    /// # Returns
    /// - `bool`: True if `get_reserves() >= amount`, false otherwise.
    pub fn has_reserves(&self, amount: &ManagedDecimal<C::Api, NumDecimals>) -> bool {
        self.get_reserves() >= *amount
    }

    /// Checks if the pool has sufficient supplied assets for a given amount.
    ///
    /// **Scope**: Validates supplied asset availability for operations like borrowing.
    ///
    /// **Goal**: Ensure the pool can support requested actions.
    ///
    /// # Arguments
    /// - `amount`: The amount to check against (`ManagedDecimal`).
    ///
    /// # Returns
    /// - `bool`: True if `supplied >= amount`, false otherwise.
    pub fn has_supplied(&self, amount: &ManagedDecimal<C::Api, NumDecimals>) -> bool {
        self.supplied >= *amount
    }

    /// Returns the available protocol revenue (minimum of revenue and reserves).
    ///
    /// **Scope**: Calculates the realizable revenue the protocol can claim.
    ///
    /// **Goal**: Ensure revenue withdrawals don’t exceed available reserves.
    ///
    /// **Formula**:
    /// - `available_revenue = min(revenue, reserves)`.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: Available revenue in pool asset_decimals.
    pub fn available_revenue(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        ManagedDecimal::from_raw_units(
            BigUint::min(
                self.revenue.into_raw_units().clone(),
                self.reserves.into_raw_units().clone(),
            ),
            self.params.asset_decimals,
        )
    }

    /// Checks if the given asset matches the pool’s asset.
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
        self.pool_asset == *asset
    }
}
