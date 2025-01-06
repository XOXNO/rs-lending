ADDRESS=erd1qqqqqqqqqqqqqpgq3rcsd0mqz5wtxx0p8yl670vzlr5h0890ah0sa3wp03
DEPLOY_TRANSACTION=$(mxpy data load --key=deployTransaction-devnet)

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/price_aggregator.wasm"

deploy() {
    mxpy --verbose contract deploy --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="deploy.json" --arguments 0x01 erd1cfyadenn4k9wndha0ljhlsdrww9k0jqafqq626hu9zt79urzvzasalgycz erd1x45vnu7shhecfz0v03qqfmy8srndch50cdx7m763p743tzlwah0sgzewlm \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    TRANSACTION=$(mxpy data parse --file="deploy.json" --expression="data['emitted_tx']['hash']")
    ADDRESS=$(mxpy data parse --file="deploy.json" --expression="data['emitted_tx']['address']")

    mxpy data store --key=address-devnet --value=${ADDRESS}
    mxpy data store --key=deployTransaction-devnet --value=${TRANSACTION}

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

upgrade() {
    mxpy contract upgrade ${ADDRESS} --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=100000000 --outfile="upgrade.json" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

# SC calls

unpause() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="unpause" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

pause() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="pause" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

FROM="str:HTM"
TO="str:USD"
PRICE=2939571259630567600000 # 55 USD

DECIMALS=18

set_pair_decimals() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setPairDecimals" --arguments ${FROM} ${TO} ${DECIMALS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

submit() { 
    timestamp=$(date +%s)
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="submit" --arguments ${FROM} ${TO} ${timestamp} ${PRICE} ${DECIMALS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
    }

getPairDecimals() {
    mxpy contract query ${ADDRESS} --function="getPairDecimals" --arguments ${FROM} ${TO} --proxy=${PROXY}
}