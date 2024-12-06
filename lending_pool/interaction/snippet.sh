ADDRESS=erd1qqqqqqqqqqqqqpgq4kqzk283c8zxhj3cltctl5v380v43w86ah0s26txgx

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/lending_pool.wasm"

LP_TEMPLATE_ADDRESS=erd1qqqqqqqqqqqqqpgqc0jgmy87kagq4p0unm4840md4adn0fa8ah0spuy2dz
AGGREGATOR_ADDR=erd1qqqqqqqqqqqqqpgq3rcsd0mqz5wtxx0p8yl670vzlr5h0890ah0sa3wp03

ACCOUNT_TOKEN_NAME="str:XOXNOLendingAccount"
ACCOUNT_TOKEN_TICKER="str:BOBERLEND"

ISSUE_COST=50000000000000000

ASSET="str:LXOXNO-a00540"
ASSET_2="str:XOXNO-589e09"
R_MAX=1000000000 # 100%
R_BASE=20000000 # 2%
R_SLOPE1=100000000 # 10%
R_SLOPE2=1000000000 # 100%
U_OPTIMAL=800000000 # 80%
RESERVE_FACTOR=300000000 # 30%
LTV=750000000 # 75%
LTV_EMODE=950000000 # 95%
LIQ_THRESOLD=800000000 # 80%
LIQ_THRESOLD_EMODE=970000000 # 97%
LIQ_BONUS=100000000 # 10%
LIQ_BONUS_EMODE=50000000 # 5%
LIQ_BASE_FEE=50000000 # 5%
BORROW_CAP=15000000000000000000000000 # 15.000.000 EGLD
SUPPLY_CAP=20000000000000000000000000 # 20.000.000 EGLD
CAN_BE_COLLATERAL=0x01
CAN_BE_BORROWED=0x01
IS_ISOLATED=0x00
DEBT_CEILING_USD=0x00
FLASH_LOAN_FEE=5000000 # 0.5%
IS_SILOED=0x00
FLASHLOAN_ENABLED=0x01
CAN_BORROW_IN_ISOLATION=0x00

deploy() {
    mxpy contract deploy --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="deploy.json" --arguments ${LP_TEMPLATE_ADDRESS} ${AGGREGATOR_ADDR} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

upgrade() {
    mxpy contract upgrade ${ADDRESS} --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="upgrade.json" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

# SC calls

registerAccountToken() {
    mxpy contract call ${ADDRESS} --recall-nonce  --gas-limit=100000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="registerAccountToken" --value=${ISSUE_COST} --arguments ${ACCOUNT_TOKEN_NAME} ${ACCOUNT_TOKEN_TICKER} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

create_pool() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=200000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="createLiquidityPool" --arguments ${ASSET} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
    ${LTV} ${LIQ_THRESOLD} ${LIQ_BONUS} ${LIQ_BASE_FEE} ${CAN_BE_COLLATERAL} ${CAN_BE_BORROWED} \
    ${IS_ISOLATED} ${DEBT_CEILING_USD} ${FLASH_LOAN_FEE} ${IS_SILOED} ${FLASHLOAN_ENABLED} ${CAN_BORROW_IN_ISOLATION} ${BORROW_CAP} ${SUPPLY_CAP} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

addEModeCategory() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="addEModeCategory" --arguments ${LTV_EMODE} ${LIQ_THRESOLD_EMODE} ${LIQ_BONUS_EMODE} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

addAssetToEModeCategory() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="addAssetToEModeCategory" --arguments ${ASSET} 0x02 0x01 0x01 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

supply() {
    method_name=str:supply
    sft_token_nonce=0x00
    sft_token_amount=10000000000000000000 # 100 XOXNO
    destination_address=${ADDRESS}
    mxpy contract call erd1x45vnu7shhecfz0v03qqfmy8srndch50cdx7m763p743tzlwah0sgzewlm --recall-nonce --gas-limit=60000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="MultiESDTNFTTransfer" --arguments ${ADDRESS} 0x02 str:BOBERLEND-e3f169 0x01 0x01 ${ASSET} ${sft_token_nonce} ${sft_token_amount} ${method_name} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

borrow() {
    method_name=str:borrow
    sft_token_nonce=0x00
    sft_token_amount=100000000000000000000 # 100 XOXNO
    mxpy contract call erd1x45vnu7shhecfz0v03qqfmy8srndch50cdx7m763p743tzlwah0sgzewlm --recall-nonce --gas-limit=60000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="MultiESDTNFTTransfer" --arguments ${ADDRESS} 0x01 str:BOBERLEND-e3f169 0x01 0x01 ${method_name} ${ASSET} ${sft_token_amount} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

withdraw() {
    method_name=str:withdraw
    sft_token_amount=11760000000000 # 100 XOXNO
    mxpy contract call erd1x45vnu7shhecfz0v03qqfmy8srndch50cdx7m763p743tzlwah0sgzewlm --recall-nonce --gas-limit=60000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="MultiESDTNFTTransfer" --arguments ${ADDRESS} 0x01 str:BOBERLEND-e3f169 0x01 0x01 ${method_name} ${ASSET} ${sft_token_amount} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

repay() {
    method_name=str:repay
    sft_token_nonce=0x00
    sft_token_amount=90000000000000000000 # 100 XOXNO
    mxpy contract call erd1x45vnu7shhecfz0v03qqfmy8srndch50cdx7m763p743tzlwah0sgzewlm --recall-nonce --gas-limit=60000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="MultiESDTNFTTransfer" --arguments ${ADDRESS} 0x01 ${ASSET} ${sft_token_nonce} ${sft_token_amount} ${method_name} 0x01 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Queries

get_pool_address() {
    mxpy contract query ${ADDRESS} --function="getPoolAddress" --arguments ${ASSET} --proxy=${PROXY}
}

getPoolAllowed() {
    mxpy contract query ${ADDRESS} --function="getPoolAllowed" --proxy=${PROXY}
}

getLtvCollateralInDollars() {
    mxpy contract query ${ADDRESS} --function="getLtvCollateralInDollars" --arguments 0x02 --proxy=${PROXY}
}


getTotalCollateralAvailable() {
    mxpy contract query ${ADDRESS} --function="getTotalCollateralAvailable" --arguments 0x02 --proxy=${PROXY}
}
