use crate::{helpers, oracle, proxies::*, storage, ERROR_TEMPLATE_EMPTY};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait FactoryModule:
    common_events::EventsModule
    + oracle::OracleModule
    + storage::LendingStorageModule
    + helpers::math::MathsModule
    + common_math::SharedMathModule
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

        let decimals = self.token_oracle(base_asset).get().decimals;

        let new_address = self
            .tx()
            .typed(proxy_pool::LiquidityPoolProxy)
            .init(
                base_asset,
                r_max,
                r_base,
                r_slope1,
                r_slope2,
                u_optimal,
                reserve_factor,
                decimals as usize,
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
            .upgrade(r_max, r_base, r_slope1, r_slope2, u_optimal, reserve_factor)
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .upgrade_async_call_and_exit();
    }
}
