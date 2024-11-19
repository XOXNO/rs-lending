use crate::{proxy_pool, ERROR_TEMPLATE_EMPTY};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait FactoryModule: crate::events::EventsModule {
    fn create_pool(
        &self,
        base_asset: &TokenIdentifier,
        r_base: &BigUint,
        r_slope1: &BigUint,
        r_slope2: &BigUint,
        u_optimal: &BigUint,
        reserve_factor: &BigUint,
        liquidation_threshold: &BigUint,
    ) -> ManagedAddress {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );

        let new_address = self
            .tx()
            .typed(proxy_pool::LiquidityPoolProxy)
            .init(
                base_asset,
                r_base,
                r_slope1,
                r_slope2,
                u_optimal,
                reserve_factor,
                liquidation_threshold,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .returns(ReturnsNewManagedAddress)
            .sync_call();

        self.add_new_market_event(
            base_asset,
            r_base,
            r_slope1,
            r_slope2,
            u_optimal,
            reserve_factor,
            liquidation_threshold,
            &new_address,
        );

        new_address
    }

    fn upgrade_pool(
        &self,
        lp_address: ManagedAddress,
        r_base: BigUint,
        r_slope1: BigUint,
        r_slope2: BigUint,
        u_optimal: BigUint,
        reserve_factor: BigUint,
        liquidation_threshold: BigUint,
    ) {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );
        self.tx()
            .to(lp_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .upgrade(
                r_base,
                r_slope1,
                r_slope2,
                u_optimal,
                reserve_factor,
                liquidation_threshold,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .upgrade_async_call_and_exit();
    }

    #[view(getLiqPoolTemplateAddress)]
    #[storage_mapper("liq_pool_template_address")]
    fn liq_pool_template_address(&self) -> SingleValueMapper<ManagedAddress>;
}
