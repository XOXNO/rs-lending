ADDRESS=erd1qqqqqqqqqqqqqpgqc0jgmy87kagq4p0unm4840md4adn0fa8ah0spuy2dz
DEPLOY_TRANSACTION=$(mxpy data load --key=deployTransaction-testnet)

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/liquidity_pool.wasm"

# init params
ASSET="str:EGLD"
R_MAX=1000000000
R_BASE=20000000
R_SLOPE1=100000000
R_SLOPE2=1000000000
U_OPTIMAL=800000000
RESERVE_FACTOR=300000000

deploy() {
    mxpy contract deploy --bytecode=${PROJECT} \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --recall-nonce --gas-limit=250000000 --outfile="deploy.json" \
    --arguments ${ASSET} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
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

LENDING_ADDRESS=erd1qqqqqqqqqqqqqpgq4kqzk283c8zxhj3cltctl5v380v43w86ah0s26txgx

upgrade_pool() {
    mxpy contract call ${LENDING_ADDRESS} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="upgradeLiquidityPool" \
    --gas-limit=50000000 --outfile="upgrade.json" \
    --arguments str:XOXNO-589e09 ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
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
