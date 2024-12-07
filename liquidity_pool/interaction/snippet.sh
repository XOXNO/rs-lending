ADDRESS=erd1qqqqqqqqqqqqqpgqlpf2f23jx29s6k7ftfccprn5wv7uccyuah0s5zvhn0
DEPLOY_TRANSACTION=$(mxpy data load --key=deployTransaction-testnet)

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/liquidity_pool.wasm"

# init params
LXOXNO_TOKEN="str:LXOXNO-a00540"
XOXNO_TOKEN="str:XOXNO-589e09"
MEX_TOKEN="str:MEX-a659d0"
WETH_TOKEN="str:WETH-bbe4ab"
USDC_TOKEN="str:USDC-350c4e"
HTM_TOKEN="str:HTM-23a1da"

R_MAX=1000000000000000000000 # 100%
R_BASE=15000000000000000000 # 2.5%
R_SLOPE1=200000000000000000000 # 15%
R_SLOPE2=1500000000000000000000 # 60%
U_OPTIMAL=750000000000000000000 # 80%
RESERVE_FACTOR=350000000000000000000 # 15%

DECIMALS=18
deploy() {
    mxpy contract deploy --bytecode=${PROJECT} \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --recall-nonce --gas-limit=250000000 --outfile="deploy.json" \
    --arguments ${ASSET} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} ${DECIMALS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

upgrade() {
    mxpy contract upgrade ${ADDRESS} \
    --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="upgrade.json" \
    --arguments ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

LENDING_ADDRESS=erd1qqqqqqqqqqqqqpgqn8hand40d5y40fzt62e8g0lrp42gvqp6ah0suf6k6q

upgrade_pool() {
    mxpy contract call ${LENDING_ADDRESS} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="upgradeLiquidityPool" \
    --gas-limit=50000000 --outfile="upgrade.json" \
    --arguments ${HTM_TOKEN} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}
# Queries

get_lend_token() {
    mxpy contract query ${ADDRESS} --function="lendToken" --proxy=${PROXY}
}

get_borrow_token() {
    mxpy contract query ${ADDRESS} --function="borrowToken" --proxy=${PROXY}
}

LP_ADDRESS=erd1qqqqqqqqqqqqqpgqjdtwdaj6h777tzjveepa6u9da66y0p5aah0spj6fag

get_deposit_rate() {
    mxpy contract query ${LP_ADDRESS} --function="getDepositRate" --proxy=${PROXY}
}

get_borrow_rate() {
    mxpy contract query ${LP_ADDRESS} --function="getBorrowRate" --proxy=${PROXY}
}

get_cap_utilisation() {
    mxpy contract query ${LP_ADDRESS} --function="getCapitalUtilisation" --proxy=${PROXY}
}
