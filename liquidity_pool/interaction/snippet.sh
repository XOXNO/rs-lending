PEM="$HOME/pems/dev.pem"

ADDRESS=$(erdpy data load --key=address-testnet)
DEPLOY_TRANSACTION=$(erdpy data load --key=deployTransaction-testnet)

PROXY=https://devnet-gateway.elrond.com
CHAIN_ID=D

PROJECT="../../liquidity_pool"

# init params
ASSET=0x544553542d333663616365
R_BASE=0
R_SLOPE1=40000000
R_SLOPE2=1000000000
U_OPTIMAL=800000000
RESERVE_FACTOR=100000000
LIQ_THRESOLD=700000000

PLAIN_TICKER=0x54455354
LEND_PREFIX=0x4c
BORROW_PREFIX=0x42

DUMMY_ADDR=erd1qqqqqqqqqqqqqpgquget4d6kuslc2rhrwvlyhx9wuaj04ppqu00sgvsmd0

ISSUE_COST=50000000000000000

GAS_LIMIT=250000000

deploy() {
    erdpy contract deploy --project=${PROJECT} \
    --recall-nonce --pem=${PEM} --gas-limit=${GAS_LIMIT} --outfile="deploy.json" \
    --arguments ${ASSET} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} ${LIQ_THRESOLD} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    TRANSACTION=$(erdpy data parse --file="deploy.json" --expression="data['emitted_tx']['hash']")
    ADDRESS=$(erdpy data parse --file="deploy.json" --expression="data['emitted_tx']['address']")

    erdpy data store --key=address-testnet --value=${ADDRESS}
    erdpy data store --key=deployTransaction-testnet --value=${TRANSACTION}

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

deploy_dummy() {
    erdpy contract deploy --project=${PROJECT} \
    --recall-nonce --pem=${PEM} --gas-limit=${GAS_LIMIT} --outfile="deploy.json" \
    --arguments 0x4142432d653233383030 10 10 10 80 5 50 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    TRANSACTION=$(erdpy data parse --file="deploy.json" --expression="data['emitted_tx']['hash']")
    ADDRESS=$(erdpy data parse --file="deploy.json" --expression="data['emitted_tx']['address']")

    erdpy data store --key=dummy_address --value=${ADDRESS}
    erdpy data store --key=deployDummy-testnet --value=${TRANSACTION}

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

upgrade_dummy() {
    erdpy contract upgrade ${DUMMY_ADDR} --project=${PROJECT} \
    --recall-nonce --pem=${PEM} --gas-limit=${GAS_LIMIT} --outfile="upgrade.json" \
    --arguments 0x4142432d653233383030 10 10 10 80 5 50 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

upgrade() {
    erdpy contract upgrade ${ADDRESS} \
    --project=${PROJECT} --recall-nonce --pem=${PEM} \
    --gas-limit=${GAS_LIMIT} --outfile="upgrade.json" \
    --arguments ${ASSET} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} ${LIQ_THRESOLD} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}


# Queries

get_lend_token() {
    erdpy contract query ${ADDRESS} --function="lendToken" --proxy=${PROXY}
}

get_borrow_token() {
    erdpy contract query ${ADDRESS} --function="borrowToken" --proxy=${PROXY}
}

LP_ADDRESS=erd1qqqqqqqqqqqqqpgqn8xx3p50927tye5n49nzspvw7qqqayjfu00s2kvxvf

get_deposit_rate() {
    erdpy contract query ${LP_ADDRESS} --function="getDepositRate" --proxy=${PROXY}
}

get_borrow_rate() {
    erdpy contract query ${LP_ADDRESS} --function="getBorrowRate" --proxy=${PROXY}
}

get_cap_utilisation() {
    erdpy contract query ${LP_ADDRESS} --function="getCapitalUtilisation" --proxy=${PROXY}
}
