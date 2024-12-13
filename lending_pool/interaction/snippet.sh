ADDRESS=erd1qqqqqqqqqqqqqpgq4kljp5lwvrg2kakhn5r8yeuj24sngf8cah0sjnz3sd

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/lending_pool.wasm"

LP_TEMPLATE_ADDRESS=erd1qqqqqqqqqqqqqpgqlpf2f23jx29s6k7ftfccprn5wv7uccyuah0s5zvhn0
AGGREGATOR_ADDR=erd1qqqqqqqqqqqqqpgq3rcsd0mqz5wtxx0p8yl670vzlr5h0890ah0sa3wp03
SAFE_PRICE_VIEW_ADDRESS=erd1qqqqqqqqqqqqqpgqcmnum66jxyfpcnvqk5eahj5n3ny4vkfn0n4szjjskv

ACCOUNT_TOKEN_NAME="str:XOXNOLendingAccount"
ACCOUNT_TOKEN_TICKER="str:BOBERLEND"

ISSUE_COST=50000000000000000

EGLD_TOKEN="str:EGLD"
LXOXNO_TOKEN="str:LXOXNO-a00540"
XOXNO_TOKEN="str:XOXNO-589e09"
MEX_TOKEN="str:MEX-a659d0"
WETH_TOKEN="str:WETH-bbe4ab"
USDC_TOKEN="str:USDC-350c4e"
HTM_TOKEN="str:HTM-23a1da"
XEGLD_TOKEN="str:XEGLD-23b511"
LP_XOXNO_TOKEN="str:XOXNOWEGLD-232308"

R_MAX=1000000000000000000000 # 100%
R_BASE=35000000000000000000 # 2.5%
R_SLOPE1=250000000000000000000 # 25%
R_SLOPE2=800000000000000000000 # 80%
U_OPTIMAL=800000000000000000000 # 92%
RESERVE_FACTOR=350000000000000000000 # 10%
LTV=550000000000000000000 # 80%
LTV_EMODE=950000000000000000000 # 95%
LIQ_THRESOLD=600000000000000000000 # 78%
LIQ_THRESOLD_EMODE=970000000000000000000 # 97%
LIQ_BONUS=100000000000000000000 # 7.5%
LIQ_BONUS_EMODE=20000000000000000000 # 2%
LIQ_BASE_FEE=100000000000000000000 # 10%
BORROW_CAP=50000000000000000000000  #  100M EGLD
SUPPLY_CAP=50000000000000000000000 #  100M EGLD
CAN_BE_COLLATERAL=0x01
CAN_BE_BORROWED=0x00
IS_ISOLATED=0x00
DEBT_CEILING_USD=0x00
FLASH_LOAN_FEE=5000000000000000000 # 0.5%
IS_SILOED=0x00
FLASHLOAN_ENABLED=0x01
CAN_BORROW_IN_ISOLATION=0x00

deploy() {
    mxpy contract deploy --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="deploy.json" --arguments ${LP_TEMPLATE_ADDRESS} ${AGGREGATOR_ADDR} ${SAFE_PRICE_VIEW_ADDRESS} \
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

setTokenOracle() {
    CONTRACT_ADDRESS=erd1qqqqqqqqqqqqqpgqkesz5whk008525zhrq45rfym4areg5ef0n4s2yzd75
    PRICING_METHOD=0
    TOKEN_TYPE=3
    SOURCE=1
    FIRST_TOLERANCE=125000000000000000000
    LAST_TOLERANCE=150000000000000000000
    DECIMALS=18
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setTokenOracle" --arguments ${LP_XOXNO_TOKEN} ${DECIMALS} ${CONTRACT_ADDRESS} \
    ${PRICING_METHOD} ${TOKEN_TYPE} ${SOURCE} ${FIRST_TOLERANCE} ${LAST_TOLERANCE} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

create_pool() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=200000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="createLiquidityPool" --arguments ${LP_XOXNO_TOKEN} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
    ${LTV} ${LIQ_THRESOLD} ${LIQ_BONUS} ${LIQ_BASE_FEE} ${CAN_BE_COLLATERAL} ${CAN_BE_BORROWED} \
    ${IS_ISOLATED} ${DEBT_CEILING_USD} ${FLASH_LOAN_FEE} ${IS_SILOED} ${FLASHLOAN_ENABLED} ${CAN_BORROW_IN_ISOLATION} ${BORROW_CAP} ${SUPPLY_CAP} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

upgrade_pool() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=200000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="upgradeLiquidityPool" --arguments ${USDC_TOKEN} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

editAssetConfig() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=200000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="editAssetConfig" --arguments ${MEX_TOKEN} \
    ${LTV} ${LIQ_THRESOLD} ${LIQ_BONUS} ${LIQ_BASE_FEE} \
    ${IS_ISOLATED} ${DEBT_CEILING_USD} ${IS_SILOED} ${FLASHLOAN_ENABLED} ${FLASH_LOAN_FEE} ${CAN_BE_COLLATERAL} ${CAN_BE_BORROWED} ${CAN_BORROW_IN_ISOLATION} ${BORROW_CAP} ${SUPPLY_CAP} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

setLiquidityPoolTemplate() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setLiquidityPoolTemplate" --arguments ${LP_TEMPLATE_ADDRESS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

setSafePriceView() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setSafePriceView" --arguments ${SAFE_PRICE_VIEW_ADDRESS} \
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
    --function="addAssetToEModeCategory" --arguments ${XOXNO_TOKEN} 0x01 0x01 0x01 \
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

getCollateralAmountForToken() {
    mxpy contract query ${ADDRESS} --function="getCollateralAmountForToken" --arguments 0x0e ${XEGLD_TOKEN} --proxy=${PROXY}
}

getBorrowAmountForToken() {
    mxpy contract query ${ADDRESS} --function="getBorrowAmountForToken" --arguments 0x0e ${XEGLD_TOKEN} --proxy=${PROXY}
}

getTotalCollateralAvailable() {
    mxpy contract query ${ADDRESS} --function="getTotalCollateralAvailable" --arguments 0x0a --proxy=${PROXY}
}

getTokenPriceData() {
    mxpy contract query ${ADDRESS} --function="getTokenPriceData" --arguments ${EGLD_TOKEN} --proxy=${PROXY}
}

getTokenPriceUSD() {
    mxpy contract query ${ADDRESS} --function="getTokenPriceUSD" --arguments ${LP_XOXNO_TOKEN} --proxy=${PROXY}
}
