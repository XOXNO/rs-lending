use crate::{helpers, oracle, proxies::*, storage};
use common_errors::ERROR_TEMPLATE_EMPTY;

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
        max_borrow_rate: &BigUint,
        base_borrow_rate: &BigUint,
        slope1: &BigUint,
        slope2: &BigUint,
        slope3: &BigUint,
        mid_utilization: &BigUint,
        optimal_utilization: &BigUint,
        reserve_factor: &BigUint,
    ) -> ManagedAddress {
        require!(
            !self.liq_pool_template_address().is_empty(),
            ERROR_TEMPLATE_EMPTY
        );

        let decimals = self.token_oracle(base_asset).get().price_decimals;

        let new_address = self
            .tx()
            .typed(proxy_pool::LiquidityPoolProxy)
            .init(
                base_asset,
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
                decimals,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE)
            .returns(ReturnsNewManagedAddress)
            .sync_call();

        new_address
    }

    fn upgrade_pool(
        &self,
        lp_address: ManagedAddress,
        max_borrow_rate: BigUint,
        base_borrow_rate: BigUint,
        slope1: BigUint,
        slope2: BigUint,
        slope3: BigUint,
        mid_utilization: BigUint,
        optimal_utilization: BigUint,
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
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
            )
            .from_source(self.liq_pool_template_address().get())
            .code_metadata(CodeMetadata::UPGRADEABLE | CodeMetadata::READABLE)
            .upgrade_async_call_and_exit();
    }
}
