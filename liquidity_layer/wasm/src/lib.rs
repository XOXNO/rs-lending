// Code generated by the multiversx-sc build system. DO NOT EDIT.

////////////////////////////////////////////////////
////////////////// AUTO-GENERATED //////////////////
////////////////////////////////////////////////////

// Init:                                 1
// Upgrade:                              1
// Endpoints:                           23
// Async Callback (empty):               1
// Total number of exported functions:  26

#![no_std]

multiversx_sc_wasm_adapter::allocator!();
multiversx_sc_wasm_adapter::panic_handler!();

multiversx_sc_wasm_adapter::endpoints! {
    liquidity_layer
    (
        init => init
        upgrade => upgrade
        getPoolAsset => pool_asset
        getReserves => reserves
        getSuppliedAmount => supplied
        getProtocolRevenue => revenue
        getTotalBorrow => borrowed
        getParams => params
        getBorrowIndex => borrow_index
        getSupplyIndex => supply_index
        getLastTimestamp => last_timestamp
        updateIndexes => update_indexes
        updatePositionInterest => sync_position_interest
        supply => supply
        borrow => borrow
        withdraw => withdraw
        repay => repay
        flashLoan => flash_loan
        createStrategy => create_strategy
        addProtocolRevenue => add_protocol_revenue
        claimRevenue => claim_revenue
        getCapitalUtilisation => get_capital_utilisation
        getTotalCapital => get_total_capital
        getDepositRate => get_deposit_rate
        getBorrowRate => get_borrow_rate
    )
}

multiversx_sc_wasm_adapter::async_callback_empty! {}
