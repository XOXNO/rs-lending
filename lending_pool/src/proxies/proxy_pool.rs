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
    /// Initializes the liquidity pool for a specific asset. 
    ///  
    /// This function sets the asset for the pool, initializes the interest rate parameters 
    /// (maximum rate, base rate, slopes, optimal utilization, reserve factor) using a given decimal precision, 
    /// and initializes both the borrow and supply indexes to the base point (BP). It also sets the protocol revenue 
    /// to zero and records the current blockchain timestamp. 
    ///  
    /// # Parameters 
    /// - `asset`: The asset identifier (EgldOrEsdtTokenIdentifier) for the pool. 
    /// - `r_max`: The maximum borrow rate. 
    /// - `r_base`: The base borrow rate. 
    /// - `r_slope1`: The slope before optimal utilization. 
    /// - `r_slope2`: The slope after optimal utilization. 
    /// - `u_optimal`: The optimal utilization ratio. 
    /// - `reserve_factor`: The fraction (reserve factor) of accrued interest reserved as protocol fee. 
    /// - `decimals`: The number of decimals for the underlying asset. 
    ///  
    /// # Returns 
    /// - Nothing. 
    pub fn init<
        Arg0: ProxyArg<EgldOrEsdtTokenIdentifier<Env::Api>>,
        Arg1: ProxyArg<BigUint<Env::Api>>,
        Arg2: ProxyArg<BigUint<Env::Api>>,
        Arg3: ProxyArg<BigUint<Env::Api>>,
        Arg4: ProxyArg<BigUint<Env::Api>>,
        Arg5: ProxyArg<BigUint<Env::Api>>,
        Arg6: ProxyArg<BigUint<Env::Api>>,
        Arg7: ProxyArg<usize>,
    >(
        self,
        asset: Arg0,
        r_max: Arg1,
        r_base: Arg2,
        r_slope1: Arg3,
        r_slope2: Arg4,
        u_optimal: Arg5,
        reserve_factor: Arg6,
        decimals: Arg7,
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
            .argument(&decimals)
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
    /// Upgrades the liquidity pool parameters. 
    ///  
    /// This function updates the pool's interest rate parameters and reserve factor. It emits an event 
    /// reflecting the new parameters, and then updates the on-chain pool parameters accordingly. 
    ///  
    /// # Parameters 
    /// - `r_max`: The new maximum borrow rate. 
    /// - `r_base`: The new base borrow rate. 
    /// - `r_slope1`: The new slope before optimal utilization. 
    /// - `r_slope2`: The new slope after optimal utilization. 
    /// - `u_optimal`: The new optimal utilization ratio. 
    /// - `reserve_factor`: The new reserve factor. 
    ///  
    /// # Returns 
    /// - Nothing. 
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
    /// Returns the pool asset identifier. 
    ///  
    /// # Returns 
    /// - `EgldOrEsdtTokenIdentifier`: The asset managed by this pool. 
    pub fn pool_asset(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, EgldOrEsdtTokenIdentifier<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getPoolAsset")
            .original_result()
    }

    /// Retrieves the current reserves available in the pool. 
    ///  
    /// Reserves represent tokens held in the pool that are available for borrowing or withdrawal. 
    ///  
    /// # Returns 
    /// - `BigUint`: The current reserves. 
    pub fn reserves(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getReserves")
            .original_result()
    }

    /// Retrieves the total amount supplied to the pool. 
    ///  
    /// # Returns 
    /// - `BigUint`: The total supplied tokens. 
    pub fn supplied_amount(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getSuppliedAmount")
            .original_result()
    }

    /// Retrieves the protocol revenue accrued from borrow interest fees. 
    ///  
    /// # Returns 
    /// - `BigUint`: The accumulated protocol revenue. 
    pub fn protocol_revenue(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getProtocolRevenue")
            .original_result()
    }

    /// Retrieves the total borrowed amount from the pool. 
    ///  
    /// # Returns 
    /// - `BigUint`: The total tokens borrowed. 
    pub fn borrowed_amount(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalBorrow")
            .original_result()
    }

    /// Returns the pool parameters. 
    ///  
    /// These include interest rate parameters and asset decimals. 
    ///  
    /// # Returns 
    /// - `PoolParams<Self::Api>`: The pool configuration. 
    pub fn pool_params(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::PoolParams<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getPoolParams")
            .original_result()
    }

    /// Retrieves the current borrow index. 
    ///  
    /// The borrow index is used to calculate accrued interest on borrow positions. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow index. 
    pub fn borrow_index(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getBorrowIndex")
            .original_result()
    }

    /// Retrieves the current supply index. 
    ///  
    /// The supply index is used to compute the yield for suppliers. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current supply index. 
    pub fn supply_index(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getSupplyIndex")
            .original_result()
    }

    /// Retrieves the last update timestamp for the interest indexes. 
    ///  
    /// # Returns 
    /// - `u64`: The timestamp when indexes were last updated. 
    pub fn last_timestamp(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, u64> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getLastUpdateTimestamp")
            .original_result()
    }

    /// Updates the market's interest indexes based on elapsed time. 
    ///  
    /// This function updates both the borrow and supply indexes. It first creates a StorageCache to read the 
    /// current state, then updates the indexes by computing an interest factor based on the elapsed time, 
    /// and finally emits a market state event. 
    ///  
    /// # Parameters 
    /// - `asset_price`: The current price of the asset. 
    ///  
    /// # Returns 
    /// - Nothing. 
    pub fn update_indexes<
        Arg0: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        asset_price: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("updateIndexes")
            .argument(&asset_price)
            .original_result()
    }

    /// Updates an account position with accrued interest. 
    ///  
    /// This function takes an `AccountPosition` (passed from the Controller SC) and updates it by applying the 
    /// accrued interest since the last update. Optionally, if an asset price is provided, it emits an event to update 
    /// the market state. 
    ///  
    /// # Parameters 
    /// - `position`: The account position to update. 
    /// - `asset_price`: An optional asset price used for updating market state events. 
    ///  
    /// # Returns 
    /// - `AccountPosition<Self::Api>`: The updated account position with accrued interest. 
    pub fn update_position_with_interest<
        Arg0: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg1: ProxyArg<OptionalValue<ManagedDecimal<Env::Api, usize>>>,
    >(
        self,
        position: Arg0,
        asset_price: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("updatePositionInterest")
            .argument(&position)
            .argument(&asset_price)
            .original_result()
    }

    /// Supplies liquidity to the pool. 
    ///  
    /// This function is called by the Controller SC to deposit assets into the market. It validates the asset, 
    /// updates the depositor's position with accrued interest, increases the pool's reserves and total supplied amount, 
    /// and emits a market state event. 
    ///  
    /// # Parameters 
    /// - `deposit_position`: The current account position of the supplier. 
    /// - `asset_price`: The current price of the asset. 
    ///  
    /// # Returns 
    /// - `AccountPosition<Self::Api>`: The updated deposit position. 
    pub fn supply<
        Arg0: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        position: Arg0,
        asset_price: Arg1,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("supply")
            .argument(&position)
            .argument(&asset_price)
            .original_result()
    }

    /// Borrows liquidity from the pool. 
    ///  
    /// This function is called by the Controller SC to borrow assets. It updates the borrower's position with accrued interest, 
    /// ensures sufficient liquidity is available, increases the total borrowed amount, deducts reserves, and transfers tokens to the borrower. 
    ///  
    /// # Parameters 
    /// - `initial_caller`: The address of the borrower. 
    /// - `borrow_amount`: The amount to borrow. 
    /// - `borrow_position`: The borrower's current account position. 
    /// - `asset_price`: The current asset price. 
    ///  
    /// # Returns 
    /// - `AccountPosition<Self::Api>`: The updated borrow position. 
    pub fn borrow<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg2: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg3: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        initial_caller: Arg0,
        borrow_amount: Arg1,
        position: Arg2,
        asset_price: Arg3,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("borrow")
            .argument(&initial_caller)
            .argument(&borrow_amount)
            .argument(&position)
            .argument(&asset_price)
            .original_result()
    }

    /// Withdraws liquidity from the pool via normal withdraw or liquidations 
    ///  
    /// # Parameters 
    /// - `initial_caller`: The address of the caller. 
    /// - `amount`: The amount of the asset to withdraw. 
    /// - `mut deposit_position`: The position to update. 
    /// - `is_liquidation`: Whether the withdrawal is part of a liquidation process. 
    /// - `protocol_liquidation_fee`: The protocol liquidation fee (if applicable, if not will be 0). 
    /// - `asset_price`: The current asset price used to update market state. 
    ///  
    /// # Returns 
    /// - `AccountPosition<Self::Api>`: The updated deposit position. 
    pub fn withdraw<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg2: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg3: ProxyArg<bool>,
        Arg4: ProxyArg<Option<ManagedDecimal<Env::Api, usize>>>,
        Arg5: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        initial_caller: Arg0,
        requested_amount: Arg1,
        deposit_position: Arg2,
        is_liquidation: Arg3,
        protocol_fee_opt: Arg4,
        asset_price: Arg5,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("withdraw")
            .argument(&initial_caller)
            .argument(&requested_amount)
            .argument(&deposit_position)
            .argument(&is_liquidation)
            .argument(&protocol_fee_opt)
            .argument(&asset_price)
            .original_result()
    }

    /// Processes a repayment for a borrow position. 
    ///  
    /// This function handles both full and partial repayments. It updates the borrower's position with accrued interest, 
    /// splits the repayment into principal and interest, issues refunds if the repayment exceeds the total debt, and 
    /// updates the pool state accordingly. 
    ///  
    /// # Parameters 
    /// - `initial_caller`: The address of the caller. 
    /// - `mut position`: The borrower's current account position. 
    /// - `asset_price`: The current asset price used for updating market state. 
    ///  
    /// # Returns 
    /// - `AccountPosition<Self::Api>`: The updated borrow position. 
    pub fn repay<
        Arg0: ProxyArg<ManagedAddress<Env::Api>>,
        Arg1: ProxyArg<common_structs::AccountPosition<Env::Api>>,
        Arg2: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        initial_caller: Arg0,
        position: Arg1,
        asset_price: Arg2,
    ) -> TxTypedCall<Env, From, To, (), Gas, common_structs::AccountPosition<Env::Api>> {
        self.wrapped_tx
            .raw_call("repay")
            .argument(&initial_caller)
            .argument(&position)
            .argument(&asset_price)
            .original_result()
    }

    /// Provides a flash loan from the pool. 
    ///  
    /// This function allows a flash loan operation. It deducts the loan amount from reserves, computes the fee, 
    /// makes an external call to the borrower's contract, verifies that the repayment (including fee) meets the minimum requirement, 
    /// and then updates the pool state accordingly. 
    ///  
    /// # Parameters 
    /// - `borrowed_token`: The token to be flash loaned (must match the pool asset). 
    /// - `amount`: The amount to flash loan. 
    /// - `contract_address`: The address of the contract to be called. 
    /// - `endpoint`: The endpoint of the target contract. 
    /// - `arguments`: The arguments to pass to the target contract. 
    /// - `fees`: The fee rate for the flash loan. 
    /// - `asset_price`: The current asset price. 
    ///  
    /// # Returns 
    /// - Nothing. 
    pub fn flash_loan<
        Arg0: ProxyArg<EgldOrEsdtTokenIdentifier<Env::Api>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg2: ProxyArg<ManagedAddress<Env::Api>>,
        Arg3: ProxyArg<ManagedBuffer<Env::Api>>,
        Arg4: ProxyArg<ManagedArgBuffer<Env::Api>>,
        Arg5: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg6: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        borrowed_token: Arg0,
        loaned_amount: Arg1,
        contract_address: Arg2,
        endpoint: Arg3,
        arguments: Arg4,
        fees: Arg5,
        asset_price: Arg6,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ()> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("flashLoan")
            .argument(&borrowed_token)
            .argument(&loaned_amount)
            .argument(&contract_address)
            .argument(&endpoint)
            .argument(&arguments)
            .argument(&fees)
            .argument(&asset_price)
            .original_result()
    }

    /// Simulates a flash loan strategy. 
    ///  
    /// This function is used internally to simulate a strategy where a flash loan is taken (without immediate repayment), 
    /// the accrued fee is added as interest to the position, and the tokens are transferred to the caller. 
    /// It returns the current borrow index and timestamp for later updates to the position. 
    ///  
    /// # Parameters 
    /// - `token`: The token identifier (must match the pool asset). 
    /// - `amount`: The amount to flash borrow for the strategy. 
    /// - `fee`: The fee for the flash loan. 
    /// - `asset_price`: The current asset price. 
    ///  
    /// # Returns 
    /// - `(BigUint, u64)`: A tuple containing the latest borrow index and the current timestamp. 
    pub fn internal_create_strategy<
        Arg0: ProxyArg<EgldOrEsdtTokenIdentifier<Env::Api>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg2: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg3: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        token: Arg0,
        strategy_amount: Arg1,
        strategy_fee: Arg2,
        asset_price: Arg3,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, (ManagedDecimal<Env::Api, usize>, u64)> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("createStrategy")
            .argument(&token)
            .argument(&strategy_amount)
            .argument(&strategy_fee)
            .argument(&asset_price)
            .original_result()
    }

    /// Adds external protocol revenue to the pool. 
    ///  
    /// This function accepts an external payment (e.g., from vault liquidations) in the pool asset, 
    /// converts it to a ManagedDecimal using the pool's decimals, and adds it to both the protocol revenue and reserves. 
    /// It then updates the market state event. 
    ///  
    /// # Parameters 
    /// - `asset_price`: The current asset price. 
    ///  
    /// # Returns 
    /// - Nothing. 
    pub fn add_external_protocol_revenue<
        Arg0: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        asset_price: Arg0,
    ) -> TxTypedCall<Env, From, To, (), Gas, ()> {
        self.wrapped_tx
            .raw_call("addExternalProtocolRevenue")
            .argument(&asset_price)
            .original_result()
    }

    /// Claims the protocol revenue. 
    ///  
    /// This function updates the market's interest indexes, calculates the available protocol revenue (by taking the minimum 
    /// of the protocol revenue and reserves), and transfers that amount to the protocol owner. 
    /// It then emits an event with the updated market state. 
    ///  
    /// # Parameters 
    /// - `asset_price`: The current asset price. 
    ///  
    /// # Returns 
    /// - `EgldOrEsdtTokenPayment<Self::Api>`: The payment representing the claimed protocol revenue. 
    pub fn claim_revenue<
        Arg0: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        asset_price: Arg0,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, EgldOrEsdtTokenPayment<Env::Api>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("claimRevenue")
            .argument(&asset_price)
            .original_result()
    }

    /// Retrieves the current capital utilization of the pool. 
    ///  
    /// Capital utilization is defined as the ratio of borrowed tokens to the total supplied tokens. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current utilization ratio. 
    pub fn get_capital_utilisation(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getCapitalUtilisation")
            .original_result()
    }

    /// Retrieves the total capital of the pool. 
    ///  
    /// Total capital is defined as the sum of reserves and borrowed tokens. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total capital. 
    pub fn get_total_capital(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getTotalCapital")
            .original_result()
    }

    /// Computes the total accrued interest on a borrow position. 
    ///  
    /// The interest is computed based on the difference between the current and the initial borrow index. 
    ///  
    /// # Parameters 
    /// - `amount`: The principal amount borrowed. 
    /// - `initial_borrow_index`: The borrow index at the time of borrowing. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The accrued interest. 
    pub fn get_debt_interest<
        Arg0: ProxyArg<ManagedDecimal<Env::Api, usize>>,
        Arg1: ProxyArg<ManagedDecimal<Env::Api, usize>>,
    >(
        self,
        amount: Arg0,
        initial_borrow_index: Arg1,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getDebtInterest")
            .argument(&amount)
            .argument(&initial_borrow_index)
            .original_result()
    }

    /// Retrieves the current deposit rate for the pool. 
    ///  
    /// The deposit rate is derived from capital utilization, the borrow rate, and the reserve factor. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current deposit rate. 
    pub fn get_deposit_rate(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getDepositRate")
            .original_result()
    }

    /// Retrieves the current borrow rate for the pool. 
    ///  
    /// # Returns 
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow rate. 
    pub fn get_borrow_rate(
        self,
    ) -> TxTypedCall<Env, From, To, NotPayable, Gas, ManagedDecimal<Env::Api, usize>> {
        self.wrapped_tx
            .payment(NotPayable)
            .raw_call("getBorrowRate")
            .original_result()
    }
}
