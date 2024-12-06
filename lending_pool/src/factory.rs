use common_events::DECIMAL_PRECISION;

use crate::{proxy_pool, ERROR_TEMPLATE_EMPTY};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait FactoryModule:
    common_events::EventsModule + crate::oracle::OracleModule + crate::storage::LendingStorageModule
{
    fn create_pool(
        &self,
        base_asset: &EgldOrEsdtTokenIdentifier,
        r_max: &BigUint,
        r_base: &BigUint,
        r_slope1: &BigUint,
        r_slope2: &BigUint,
        u_optimal: &BigUint,
        reserve_factor: &BigUint,
    ) -> ManagedAddress {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );
        let decimals = self.get_token_price_data(base_asset);
        let new_address = self
            .tx()
            .typed(proxy_pool::LiquidityPoolProxy)
            .init(
                base_asset,
                ManagedDecimal::from_raw_units(r_max.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_base.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_slope1.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_slope2.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(u_optimal.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(reserve_factor.clone(), DECIMAL_PRECISION),
                decimals.decimals as usize,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .returns(ReturnsNewManagedAddress)
            .sync_call();

        new_address
    }

    fn upgrade_pool(
        &self,
        lp_address: ManagedAddress,
        r_max: BigUint,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
    ) {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );
        self.tx()
            .to(lp_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .upgrade(
                ManagedDecimal::from_raw_units(r_max.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_base.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_slope1.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(r_slope2.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(u_optimal.clone(), DECIMAL_PRECISION),
                ManagedDecimal::from_raw_units(reserve_factor.clone(), DECIMAL_PRECISION),
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .upgrade_async_call_and_exit();
    }

    #[view(getLiqPoolTemplateAddress)]
    #[storage_mapper("liq_pool_template_address")]
    fn liq_pool_template_address(&self) -> SingleValueMapper<ManagedAddress>;
}
