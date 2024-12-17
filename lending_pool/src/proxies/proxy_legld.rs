// Code generated by the multiversx-sc proxy generator. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![allow(dead_code)]
#![allow(clippy::all)]

use multiversx_sc::proxy_imports::*;

pub struct SalsaContractProxy;

impl<Env, From, To, Gas> TxProxyTrait<Env, From, To, Gas> for SalsaContractProxy
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    type TxProxyMethods = SalsaContractProxyMethods<Env, From, To, Gas>;

    fn proxy_methods(self, tx: Tx<Env, From, To, (), Gas, (), ()>) -> Self::TxProxyMethods {
        SalsaContractProxyMethods { wrapped_tx: tx }
    }
}

pub struct SalsaContractProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    wrapped_tx: Tx<Env, From, To, (), Gas, (), ()>,
}

#[rustfmt::skip]
impl<Env, From, Gas> SalsaContractProxyMethods<Env, From, (), Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    Gas: TxGas<Env>,
{
    pub fn init(
        self,
    ) -> TxTypedDeploy<Env, From, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_deploy()
            .original_result()
    }
}

#[rustfmt::skip]
impl<Env, From, To, Gas> SalsaContractProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    pub fn upgrade(
        self,
    ) -> TxTypedUpgrade<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_upgrade()
            .original_result()
    }
}

#[rustfmt::skip]
impl<Env, From, To, Gas> SalsaContractProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    ///     * Delegate\n      
    pub fn delegate<
        Arg0: ProxyArg<bool>,
        Arg1: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        with_custody: Arg0,
        without_arbitrage: Arg1,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("delegate")
            .argument(&with_custody)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Undelegate\n      
    pub fn undelegate<
        Arg0: ProxyArg<Option<BigUint<Env::Api>>>,
        Arg1: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        undelegate_amount: Arg0,
        without_arbitrage: Arg1,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("unDelegate")
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Withdraw\n      
    pub fn withdraw(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("withdraw")
            .original_result()
    }

    ///     * Add to custody\n      
    pub fn add_to_custody<
        Arg0: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        without_arbitrage: Arg0,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("addToCustody")
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Remove from custody\n      
    pub fn remove_from_custody<
        Arg0: ProxyArg<BigUint<Env::Api>>,
        Arg1: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        amount: Arg0,
        without_arbitrage: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeFromCustody")
            .argument(&amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Add reserve\n      
    pub fn add_reserve<
        Arg0: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        without_arbitrage: Arg0,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("addReserve")
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Remove reserve\n      
    pub fn remove_reserve<
        Arg0: ProxyArg<BigUint<Env::Api>>,
        Arg1: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        amount: Arg0,
        without_arbitrage: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeReserve")
            .argument(&amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Undelegate now\n      
    pub fn undelegate_now<
        Arg0: ProxyArg<BigUint<Env::Api>>,
        Arg1: ProxyArg<Option<BigUint<Env::Api>>>,
        Arg2: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        min_amount_out: Arg0,
        undelegate_amount: Arg1,
        without_arbitrage: Arg2,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("unDelegateNow")
            .argument(&min_amount_out)
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    pub fn undelegate_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        undelegate_amount: Arg1,
        without_arbitrage: Arg2,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("unDelegateKnight")
            .argument(&user)
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Undelegate now knight\n      
    pub fn undelegate_now_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<BigUint<Env::Api>>,
        Arg3: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        min_amount_out: Arg1,
        undelegate_amount: Arg2,
        without_arbitrage: Arg3,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("unDelegateNowKnight")
            .argument(&user)
            .argument(&min_amount_out)
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Withdraw knight\n      
    pub fn withdraw_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("withdrawKnight")
            .argument(&user)
            .original_result()
    }

    ///     * Remove reserve knight\n      
    pub fn remove_reserve_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        amount: Arg1,
        without_arbitrage: Arg2,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeReserveKnight")
            .argument(&user)
            .argument(&amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Undelegate heir\n      
    pub fn undelegate_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        undelegate_amount: Arg1,
        without_arbitrage: Arg2,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("unDelegateHeir")
            .argument(&user)
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Undelegate now heir\n      
    pub fn undelegate_now_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<BigUint<Env::Api>>,
        Arg3: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        min_amount_out: Arg1,
        undelegate_amount: Arg2,
        without_arbitrage: Arg3,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("unDelegateNowHeir")
            .argument(&user)
            .argument(&min_amount_out)
            .argument(&undelegate_amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Withdraw heir\n      
    pub fn withdraw_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("withdrawHeir")
            .argument(&user)
            .original_result()
    }

    ///     * Remove reserve heir\n      
    pub fn remove_reserve_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<OptionalValue<bool>>,
    >(
        self,
        user: Arg0,
        amount: Arg1,
        without_arbitrage: Arg2,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeReserveHeir")
            .argument(&user)
            .argument(&amount)
            .argument(&without_arbitrage)
            .original_result()
    }

    ///     * Helpers\n      
    pub fn call_reduce_egld_to_delegate_undelegate(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("reduceEgldToDelegateUndelegate")
            .original_result()
    }

    pub fn register_liquid_token<
        Arg0: ProxyArg<ManagedBuffer<Env::Api>>,
        Arg1: ProxyArg<ManagedBuffer<Env::Api>>,
        Arg2: ProxyArg<usize>,
    >(
        self,
        token_display_name: Arg0,
        token_ticker: Arg1,
        num_decimals: Arg2,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("registerLiquidToken")
            .argument(&token_display_name)
            .argument(&token_ticker)
            .argument(&num_decimals)
            .original_result()
    }

    pub fn liquid_token_id(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, TokenIdentifier<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getLiquidTokenId")
            .original_result()
    }

    pub fn liquid_token_supply(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getLiquidTokenSupply")
            .original_result()
    }

    pub fn set_state_active(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setStateActive")
            .original_result()
    }

    pub fn set_state_inactive(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setStateInactive")
            .original_result()
    }

    pub fn state(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, State> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getState")
            .original_result()
    }

    pub fn unbond_period(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUnbondPeriod")
            .original_result()
    }

    pub fn set_unbond_period<
        Arg0: ProxyArg<u64>,
    >(
        self,
        period: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setUnbondPeriod")
            .argument(&period)
            .original_result()
    }

    pub fn service_fee(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getServiceFee")
            .original_result()
    }

    pub fn set_service_fee<
        Arg0: ProxyArg<u64>,
    >(
        self,
        new_fee: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setServiceFee")
            .argument(&new_fee)
            .original_result()
    }

    pub fn max_provider_fee(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getMaxProviderFee")
            .original_result()
    }

    pub fn set_max_provider_fee<
        Arg0: ProxyArg<u64>,
    >(
        self,
        new_fee: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setMaxProviderFee")
            .argument(&new_fee)
            .original_result()
    }

    pub fn luser_undelegations<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, Undelegation<Env::Api>>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUserUndelegations")
            .argument(&user)
            .original_result()
    }

    pub fn total_egld_staked(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalEgldStaked")
            .original_result()
    }

    pub fn user_withdrawn_egld(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUserWithdrawnEgld")
            .original_result()
    }

    pub fn total_withdrawn_egld(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalWithdrawnEgld")
            .original_result()
    }

    pub fn ltotal_user_undelegations(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, Undelegation<Env::Api>>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalUserUndelegations")
            .original_result()
    }

    pub fn egld_reserve(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getEgldReserve")
            .original_result()
    }

    pub fn reserve_points(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReservePoints")
            .original_result()
    }

    pub fn available_egld_reserve(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getAvailableEgldReserve")
            .original_result()
    }

    pub fn lreserve_undelegations(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, Undelegation<Env::Api>>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReserveUndelegations")
            .original_result()
    }

    pub fn users_reserve_points<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUsersReservePoints")
            .argument(&user)
            .original_result()
    }

    pub fn set_undelegate_now_fee<
        Arg0: ProxyArg<u64>,
    >(
        self,
        new_fee: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setUndelegateNowFee")
            .argument(&new_fee)
            .original_result()
    }

    pub fn undelegate_now_fee(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUndelegateNowFee")
            .original_result()
    }

    pub fn get_reserve_points_amount<
        Arg0: ProxyArg<BigUint<Env::Api>>,
    >(
        self,
        egld_amount: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReservePointsAmount")
            .argument(&egld_amount)
            .original_result()
    }

    pub fn get_reserve_egld_amount<
        Arg0: ProxyArg<BigUint<Env::Api>>,
    >(
        self,
        points_amount: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReserveEgldAmount")
            .argument(&points_amount)
            .original_result()
    }

    pub fn get_user_reserve<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUserReserve")
            .argument(&user)
            .original_result()
    }

    pub fn token_price(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTokenPrice")
            .original_result()
    }

    pub fn view_provider_updated<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        provider_address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, bool> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("isProviderUpToDate")
            .argument(&provider_address)
            .original_result()
    }

    pub fn view_providers_updated(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, bool> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("areProvidersUpToDate")
            .original_result()
    }

    pub fn set_wrap_sc<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setWrapSC")
            .argument(&address)
            .original_result()
    }

    pub fn legld_in_custody(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getLegldInCustody")
            .original_result()
    }

    pub fn user_delegation<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUserDelegation")
            .argument(&user)
            .original_result()
    }

    pub fn knight_users<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        knight: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, ManagedAddress<Env::Api>>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getKnightUsers")
            .argument(&knight)
            .original_result()
    }

    pub fn user_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, Heir<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getUserHeir")
            .argument(&user)
            .original_result()
    }

    pub fn heir_users<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        heir: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, ManagedAddress<Env::Api>>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getHeirUsers")
            .argument(&heir)
            .original_result()
    }

    pub fn delegate_all(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("delegateAll")
            .original_result()
    }

    pub fn undelegate_all(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("unDelegateAll")
            .original_result()
    }

    pub fn claim_rewards(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("claimRewards")
            .original_result()
    }

    pub fn withdraw_all<
        Arg0: ProxyArg<Option<u64>>,
        Arg1: ProxyArg<MultiValueEncoded<Env::Api, ManagedAddress<Env::Api>>>,
    >(
        self,
        gas: Arg0,
        providers_to_withdraw_from: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("withdrawAll")
            .argument(&gas)
            .argument(&providers_to_withdraw_from)
            .original_result()
    }

    pub fn compute_withdrawn(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("computeWithdrawn")
            .original_result()
    }

    pub fn set_arbitrage_active(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setArbitrageActive")
            .original_result()
    }

    pub fn set_arbitrage_inactive(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setArbitrageInactive")
            .original_result()
    }

    pub fn arbitrage(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, State> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getArbitrageState")
            .original_result()
    }

    ///     * Trigger arbitrage\n      
    pub fn trigger_arbitrage(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("triggerArbitrage")
            .original_result()
    }

    pub fn set_onedex_arbitrage_active(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setOnedexArbitrageActive")
            .original_result()
    }

    pub fn set_onedex_arbitrage_inactive(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setOnedexArbitrageInactive")
            .original_result()
    }

    pub fn onedex_arbitrage(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, State> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getOnedexArbitrageState")
            .original_result()
    }

    pub fn set_onedex_sc<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setOnedexSC")
            .argument(&address)
            .original_result()
    }

    pub fn set_onedex_pair_id<
        Arg0: ProxyArg<usize>,
    >(
        self,
        id: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setOnedexPairId")
            .argument(&id)
            .original_result()
    }

    pub fn set_xexchange_arbitrage_active(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setXexchangeArbitrageActive")
            .original_result()
    }

    pub fn set_xexchange_arbitrage_inactive(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setXexchangeArbitrageInactive")
            .original_result()
    }

    pub fn xexchange_arbitrage(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, State> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getXexchangeArbitrageState")
            .original_result()
    }

    pub fn set_xexchange_sc<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setXexchangeSC")
            .argument(&address)
            .original_result()
    }

    pub fn set_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        knight: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setKnight")
            .argument(&knight)
            .original_result()
    }

    pub fn cancel_knight(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("cancelKnight")
            .original_result()
    }

    pub fn activate_knight(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("activateKnight")
            .original_result()
    }

    pub fn deactivate_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("deactivateKnight")
            .argument(&user)
            .original_result()
    }

    pub fn confirm_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("confirmKnight")
            .argument(&user)
            .original_result()
    }

    pub fn remove_knight<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeKnight")
            .argument(&user)
            .original_result()
    }

    pub fn set_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<u64>,
    >(
        self,
        heir: Arg0,
        inheritance_epochs: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setHeir")
            .argument(&heir)
            .argument(&inheritance_epochs)
            .original_result()
    }

    pub fn cancel_heir(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("cancelHeir")
            .original_result()
    }

    pub fn remove_heir<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        user: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeHeir")
            .argument(&user)
            .original_result()
    }

    pub fn update_last_accessed(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("updateLastAccessed")
            .original_result()
    }

    pub fn add_provider<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("addProvider")
            .argument(&address)
            .original_result()
    }

    pub fn refresh_provider<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("refreshProvider")
            .argument(&address)
            .original_result()
    }

    pub fn remove_provider<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
    >(
        self,
        address: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("removeProvider")
            .argument(&address)
            .original_result()
    }

    pub fn set_provider_state<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<State>,
    >(
        self,
        address: Arg0,
        new_state: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("setProviderState")
            .argument(&address)
            .argument(&new_state)
            .original_result()
    }

    ///     * Refresh Providers - updates all providers infos and returns true if all active providers are up-to-date and\n     * at least one provider is active, and false otherwise\n      
    pub fn refresh_providers(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, bool> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("refreshProviders")
            .original_result()
    }
}

#[type_abi]
#[derive(TopEncode, TopDecode)]
pub enum State {
    Inactive,
    Active,
}


#[type_abi]
#[derive(TopEncode, TopDecode)]
pub struct Undelegation<Api>
where
    Api: ManagedTypeApi,
{
    pub amount: BigUint<Api>,
    pub unbond_epoch: u64,
}


#[type_abi]
#[derive(TopEncode, TopDecode)]
pub enum KnightState {
    Undefined,
    InactiveKnight,
    PendingConfirmation,
    ActiveKnight,
}

#[type_abi]
#[derive(TopEncode, TopDecode)]
pub struct Heir<Api>
where
    Api: ManagedTypeApi,
{
    pub address: ManagedAddress<Api>,
    pub inheritance_epochs: u64,
    pub last_accessed_epoch: u64,
}