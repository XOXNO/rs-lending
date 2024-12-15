// Code generated by the multiversx-sc build system. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

// Init:                                 1
// Upgrade:                              1
// Endpoints:                           24
// Async Callback (empty):               1
// Total number of exported functions:  27

#![no_std]

multiversx_sc_wasm_adapter::allocator!();
multiversx_sc_wasm_adapter::panic_handler!();

multiversx_sc_wasm_adapter::endpoints! {
    liquidity_pool
    (
        init => init
        upgrade => upgrade
        getPoolAsset => pool_asset
        getReserves => reserves
        getSuppliedAmount => supplied_amount
        getRewardsReserves => protocol_revenue
        getTotalBorrow => borrowed_amount
        getPoolParams => pool_params
        getBorrowIndex => borrow_index
        getSupplyIndex => supply_index
        getLastUpdateTimestamp => last_update_timestamp
        getAccountToken => account_token
        getAccountPositions => account_positions
        updateIndexes => update_indexes
        updatePositionInterest => update_position_with_interest
        supply => supply
        borrow => borrow
        withdraw => withdraw
        repay => repay
        vaultRewards => vault_rewards
        flashLoan => flash_loan
        getCapitalUtilisation => get_capital_utilisation
        getTotalCapital => get_total_capital
        getDebtInterest => get_debt_interest
        getDepositRate => get_deposit_rate
        getBorrowRate => get_borrow_rate
    )
}

multiversx_sc_wasm_adapter::async_callback_empty! {}
