use common_structs::PoolParams;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    sc_ref: &'a C,
    /// The amount of the asset supplied.
    pub supplied_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved.
    pub reserves_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset borrowed.
    pub borrowed_amount: ManagedDecimal<C::Api, NumDecimals>,
    /// The amount of the asset reserved for protocol revenue.
    pub protocol_revenue: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the block.
    pub timestamp: u64,
    /// The asset of the pool.
    pub pool_asset: EgldOrEsdtTokenIdentifier<C::Api>,
    /// The parameters of the pool.
    pub pool_params: PoolParams<C::Api>,
    /// The borrow index.
    pub borrow_index: ManagedDecimal<C::Api, NumDecimals>,
    /// The supply index.
    pub supply_index: ManagedDecimal<C::Api, NumDecimals>,
    // Zero value
    pub zero: ManagedDecimal<C::Api, NumDecimals>,
    pub ray: ManagedDecimal<C::Api, NumDecimals>,
    /// The timestamp of the last update.
    pub last_timestamp: u64,
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::storage::StorageModule + common_math::SharedMathModule,
{
    /// Constructs a new StorageCache by reading the current state from on-chain storage.
    ///
    /// # Parameters
    /// - `sc_ref`: A reference to the contract implementing StorageModule.
    ///
    /// # Returns
    /// - `StorageCache<Self>`: A new instance containing cached market state.
    pub fn new(sc_ref: &'a C) -> Self {
        let params = sc_ref.pool_params().get();
        StorageCache {
            zero: ManagedDecimal::from_raw_units(BigUint::zero(), params.decimals),
            ray: sc_ref.ray(),
            supplied_amount: sc_ref.supplied_amount().get(),
            reserves_amount: sc_ref.reserves().get(),
            borrowed_amount: sc_ref.borrowed_amount().get(),
            protocol_revenue: sc_ref.protocol_revenue().get(),
            timestamp: sc_ref.blockchain().get_block_timestamp(),
            pool_asset: sc_ref.pool_asset().get(),
            pool_params: params,
            borrow_index: sc_ref.borrow_index().get(),
            supply_index: sc_ref.supply_index().get(),
            last_timestamp: sc_ref.last_timestamp().get(),
            sc_ref,
        }
    }
}

impl<C> Drop for StorageCache<'_, C>
where
    C: crate::storage::StorageModule,
{
    fn drop(&mut self) {
        // commit changes to storage for the mutable fields
        self.sc_ref.supplied_amount().set(&self.supplied_amount);
        self.sc_ref.reserves().set(&self.reserves_amount);
        self.sc_ref.borrowed_amount().set(&self.borrowed_amount);
        self.sc_ref.protocol_revenue().set(&self.protocol_revenue);
        self.sc_ref.borrow_index().set(&self.borrow_index);
        self.sc_ref.supply_index().set(&self.supply_index);
        self.sc_ref
            .last_timestamp()
            .set(&self.last_timestamp);
    }
}

impl<'a, C> StorageCache<'a, C>
where
    C: crate::storage::StorageModule,
{
    /// Converts a raw BigUint value into a ManagedDecimal using the pool's decimal precision.
    ///
    /// # Parameters
    /// - `value`: The raw BigUint value from storage.
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The converted decimal value.
    pub fn get_decimal_value(
        &self,
        value: &BigUint<C::Api>,
    ) -> ManagedDecimal<C::Api, NumDecimals> {
        ManagedDecimal::from_raw_units(value.clone(), self.pool_params.decimals)
    }

    /// Computes the effective reserves available (reserves minus protocol revenue).
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The available reserves.
    pub fn get_reserves(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        if self.reserves_amount >= self.protocol_revenue {
            self.reserves_amount.clone() - self.protocol_revenue.clone()
        } else {
            ManagedDecimal::from_raw_units(BigUint::zero(), self.pool_params.decimals)
        }
    }

    /// Returns the available protocol revenue (minimum of protocol revenue and reserves).
    ///
    /// # Returns
    /// - `ManagedDecimal<C::Api, NumDecimals>`: The available protocol revenue.
    pub fn available_revenue(&self) -> ManagedDecimal<C::Api, NumDecimals> {
        ManagedDecimal::from_raw_units(
            BigUint::min(
                self.protocol_revenue.into_raw_units().clone(),
                self.reserves_amount.into_raw_units().clone(),
            ),
            self.pool_params.decimals,
        )
    }
}
