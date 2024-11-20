// Code generated by the multiversx-sc proxy generator. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

#![allow(dead_code)]
#![allow(clippy::all)]

use multiversx_sc::proxy_imports::*;

pub struct LiquidityPoolProxy;

impl<Env, From, To, Gas> TxProxyTrait<Env, From, To, Gas> for LiquidityPoolProxy
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    type TxProxyMethods = LiquidityPoolProxyMethods<Env, From, To, Gas>;

    fn proxy_methods(self, tx: Tx<Env, From, To, (), Gas, (), ()>) -> Self::TxProxyMethods {
        LiquidityPoolProxyMethods { wrapped_tx: tx }
    }
}

pub struct LiquidityPoolProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    wrapped_tx: Tx<Env, From, To, (), Gas, (), ()>,
}

#[rustfmt::skip]
impl<Env, From, Gas> LiquidityPoolProxyMethods<Env, From, (), Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    Gas: TxGas<Env>,
{
    pub fn init<
        Arg0: ProxyArg<TokenIdentifier<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<BigUint<Env::Api>>,
        Arg3: ProxyArg<BigUint<Env::Api>>,
        Arg4: ProxyArg<BigUint<Env::Api>>,
        Arg5: ProxyArg<BigUint<Env::Api>>,
        Arg6: ProxyArg<BigUint<Env::Api>>,
    >(
        self,
        asset: Arg0,
        r_max: Arg1,
        r_base: Arg2,
        r_slope1: Arg3,
        r_slope2: Arg4,
        u_optimal: Arg5,
        reserve_factor: Arg6,
    ) -> TxTypedDeploy<Env, From, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_deploy()
            .argument(&asset)
            .argument(&r_max)
            .argument(&r_base)
            .argument(&r_slope1)
            .argument(&r_slope2)
            .argument(&u_optimal)
            .argument(&reserve_factor)
            .original_result()
    }
}

#[rustfmt::skip]
impl<Env, From, To, Gas> LiquidityPoolProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    pub fn upgrade<
        Arg0: ProxyArg<BigUint<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<BigUint<Env::Api>>,
        Arg3: ProxyArg<BigUint<Env::Api>>,
        Arg4: ProxyArg<BigUint<Env::Api>>,
        Arg5: ProxyArg<BigUint<Env::Api>>,
    >(
        self,
        r_max: Arg0,
        r_base: Arg1,
        r_slope1: Arg2,
        r_slope2: Arg3,
        u_optimal: Arg4,
        reserve_factor: Arg5,
    ) -> TxTypedUpgrade<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_upgrade()
            .argument(&r_max)
            .argument(&r_base)
            .argument(&r_slope1)
            .argument(&r_slope2)
            .argument(&u_optimal)
            .argument(&reserve_factor)
            .original_result()
    }
}

#[rustfmt::skip]
impl<Env, From, To, Gas> LiquidityPoolProxyMethods<Env, From, To, Gas>
where
    Env: TxEnv,
    Env::Api: VMApi,
    From: TxFrom<Env>,
    To: TxTo<Env>,
    Gas: TxGas<Env>,
{
    pub fn pool_asset(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, TokenIdentifier<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getPoolAsset")
            .original_result()
    }

    pub fn reserves(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReserves")
            .original_result()
    }

    pub fn supplied_amount(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getSuppliedAmount")
            .original_result()
    }

    pub fn rewards_reserves(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getRewardsReserves")
            .original_result()
    }

    pub fn borrowed_amount(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalBorrow")
            .original_result()
    }

    pub fn pool_params(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::PoolParams<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getPoolParams")
            .original_result()
    }

    pub fn borrow_index(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getBorrowIndex")
            .original_result()
    }

    pub fn supply_index(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getSupplyIndex")
            .original_result()
    }

    pub fn borrow_index_last_update_round(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("borrowIndexLastUpdateRound")
            .original_result()
    }

    pub fn account_token(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, TokenIdentifier<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getAccountToken")
            .original_result()
    }

    pub fn account_positions(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, MultiValueEncoded<Env::Api, u64>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getAccountPositions")
            .original_result()
    }

    pub fn update_collateral_with_interest<
        Arg0: ProxyArg<common_structs::AccountPosition<Env::Api>>,
    >(
        self,
        deposit_position: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("updatePositionInterest")
            .argument(&deposit_position)
            .original_result()
    }

    pub fn update_borrows_with_debt<
        Arg0: ProxyArg<common_structs::AccountPosition<Env::Api>>,
    >(
        self,
        borrow_position: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("updatePositionDebt")
            .argument(&borrow_position)
            .original_result()
    }

    pub fn supply<
        Arg0: ProxyArg<common_structs::AccountPosition<Env::Api>>,
    >(
        self,
        deposit_position: Arg0,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("supply")
            .argument(&deposit_position)
            .original_result()
    }

    pub fn borrow<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<common_structs::AccountPosition<Env::Api>>,
    >(
        self,
        initial_caller: Arg0,
        borrow_amount: Arg1,
        existing_borrow_position: Arg2,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("borrow")
            .argument(&initial_caller)
            .argument(&borrow_amount)
            .argument(&existing_borrow_position)
            .original_result()
    }

    pub fn withdraw<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg3: ProxyArg<bool>,
    >(
        self,
        initial_caller: Arg0,
        amount: Arg1,
        deposit_position: Arg2,
        is_liquidation: Arg3,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("withdraw")
            .argument(&initial_caller)
            .argument(&amount)
            .argument(&deposit_position)
            .argument(&is_liquidation)
            .original_result()
    }

    pub fn repay<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<common_structs::AccountPosition<Env::Api>>,
    >(
        self,
        initial_caller: Arg0,
        borrow_position: Arg1,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("repay")
            .argument(&initial_caller)
            .argument(&borrow_position)
            .original_result()
    }

    pub fn get_capital_utilisation(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getCapitalUtilisation")
            .original_result()
    }

    pub fn get_total_capital(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalCapital")
            .original_result()
    }

    pub fn get_debt_interest<
        Arg0: ProxyArg<BigUint<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
    >(
        self,
        amount: Arg0,
        initial_borrow_index: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getDebtInterest")
            .argument(&amount)
            .argument(&initial_borrow_index)
            .original_result()
    }

    pub fn get_deposit_rate(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getDepositRate")
            .original_result()
    }

    pub fn get_borrow_rate(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, BigUint<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getBorrowRate")
            .original_result()
    }
}
