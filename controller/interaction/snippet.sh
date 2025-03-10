ADDRESS=erd1qqqqqqqqqqqqqpgqhwy4zwanu52u5f39sg7arf3mmdspecumah0sha3wfd

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/controller.wasm"

LP_TEMPLATE_ADDRESS=erd1qqqqqqqqqqqqqpgqqgds0xmng8r7389jkkze7cleypnt93xqah0snz5qln
AGGREGATOR_ADDR=erd1qqqqqqqqqqqqqpgq7a48t570jjudy0xjxhuzcdwndcq9gt2tah0s7tg84a
SAFE_PRICE_VIEW_ADDRESS=erd1qqqqqqqqqqqqqpgqcmnum66jxyfpcnvqk5eahj5n3ny4vkfn0n4szjjskv
ACCUMULATOR_ADDRESS=erd1qqqqqqqqqqqqqpgqyxfc4r5fmw2ljcgwxj2nuzv72y9ryvyhah0sgn5vv2
WEGLD_ADDRESS=erd1qqqqqqqqqqqqqpgqpv09kfzry5y4sj05udcngesat07umyj70n4sa2c0rp
ASH_SWAP_ADDRESS=erd1qqqqqqqqqqqqqpgqydv5wa99jnhwtls28vtpwww8mfuvjdsqah0sre0ltj

ACCOUNT_TOKEN_NAME="str:XOXNOLendingAccount"
ACCOUNT_TOKEN_TICKER="str:BOBERLEND"

ISSUE_COST=50000000000000000

EGLD_TOKEN="str:EGLD"
LXOXNO_TOKEN="str:LXOXNO-a00540"
XOXNO_TOKEN="str:XOXNO-589e09"
MEX_TOKEN="str:MEX-a659d0"
WETH_TOKEN="str:WETH-bbe4ab"
USDC_TOKEN="str:USDC-350c4e"
USDT_TOKEN="str:USDT-58d5d0"
HTM_TOKEN="str:HTM-23a1da"
WBTC_TOKEN="str:WBTC-05fd5b"
XEGLD_TOKEN="str:XEGLD-23b511"
LP_XOXNO_TOKEN="str:XOXNOWEGLD-232308"
LEGLD_TOKEN="str:LEGLD-e8378b"
LP_XEGLD_TOKEN="str:XEGLDWEGLD-cc2dd9"

R_MAX=1000000000000000000000 # 100%
R_BASE=35000000000000000000 # 2.5%
R_SLOPE1=250000000000000000000 # 25%
R_SLOPE2=800000000000000000000 # 80%
U_OPTIMAL=800000000000000000000 # 92%
RESERVE_FACTOR=300000000000000000000 # 10%
LTV=750000000000000000000 # 80%
LTV_EMODE=9500 # 95%
LIQ_THRESOLD=780000000000000000000 # 78%
LIQ_THRESOLD_EMODE=9700 # 97%
LIQ_BONUS=100000000000000000000 # 7.5%
LIQ_BONUS_EMODE=150 # 1.5%
LIQ_BASE_FEE=100000000000000000000 # 10%
BORROW_CAP=50000000000000000000000  #  100M EGLD
SUPPLY_CAP=50000000000000000000000 #  100M EGLD
CAN_BE_COLLATERAL=0x01
CAN_BE_BORROWED=0x01
IS_ISOLATED=0x00
DEBT_CEILING_USD=0x00
FLASH_LOAN_FEE=5000000000000000000 # 0.5%
IS_SILOED=0x00
FLASHLOAN_ENABLED=0x01
CAN_BORROW_IN_ISOLATION=0x00

deploy() {
    mxpy contract deploy --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="deploy.json" --arguments ${LP_TEMPLATE_ADDRESS} ${AGGREGATOR_ADDR} ${SAFE_PRICE_VIEW_ADDRESS} ${ACCUMULATOR_ADDRESS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    echo ""
    echo "Smart contract address: ${ADDRESS}"
}

upgrade() {
    mxpy contract upgrade ${ADDRESS} --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 --arguments ${WEGLD_ADDRESS} \
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
    CONTRACT_ADDRESS=erd1qqqqqqqqqqqqqpgqe2hgwvy3mnxhr29em0243zashhy470lvvcqsf5fnxr
    PRICING_METHOD=0
    TOKEN_TYPE=2
    SOURCE=4
    FIRST_TOLERANCE=125000000000000000000
    LAST_TOLERANCE=150000000000000000000
    DECIMALS=18
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setTokenOracle" --arguments ${LEGLD_TOKEN} ${DECIMALS} ${CONTRACT_ADDRESS} \
    ${PRICING_METHOD} ${TOKEN_TYPE} ${SOURCE} ${FIRST_TOLERANCE} ${LAST_TOLERANCE} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

create_pool() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=200000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="createLiquidityPool" --arguments ${LEGLD_TOKEN} ${R_MAX} ${R_BASE} ${R_SLOPE1} ${R_SLOPE2} ${U_OPTIMAL} ${RESERVE_FACTOR} \
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

setAccumulator() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setAccumulator" --arguments ${ACCUMULATOR_ADDRESS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

setAggregator() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setAggregator" --arguments ${AGGREGATOR_ADDR} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

setAshSwap() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setAshSwap" --arguments ${ASH_SWAP_ADDRESS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

addEModeCategory() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="addEModeCategory" --arguments ${LTV_EMODE} ${LIQ_THRESOLD_EMODE} ${LIQ_BONUS_EMODE} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

removeEModeCategory() {
  mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="removeEModeCategory" --arguments 0x05 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

addAssetToEModeCategory() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="addAssetToEModeCategory" --arguments ${EGLD_TOKEN} 0x02 0x01 0x01 \
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

# ${XEGLD_TOKEN} ${XOXNO_TOKEN} ${MEX_TOKEN} ${WETH_TOKEN} ${USDC_TOKEN} ${HTM_TOKEN} ${LP_XOXNO_TOKEN}
claimRevenue() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=600000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="claimRevenue" --arguments ${EGLD_TOKEN} ${LEGLD_TOKEN} ${XEGLD_TOKEN} ${XOXNO_TOKEN} ${LXOXNO_TOKEN} ${MEX_TOKEN} ${WETH_TOKEN} ${WBTC_TOKEN} ${USDC_TOKEN} ${USDT_TOKEN} ${HTM_TOKEN} ${LP_XOXNO_TOKEN} ${LP_XEGLD_TOKEN} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Queries

get_pool_address() {
    mxpy contract query ${ADDRESS} --function="getPoolAddress" --arguments ${ASSET} --proxy=${PROXY}
}

getPoolAllowed() {
    mxpy contract query ${ADDRESS} --function="getPoolAllowed" --proxy=${PROXY}
}

getLtvCollateralInEgld() {
    mxpy contract query ${ADDRESS} --function="getLtvCollateralInEgld" --arguments 0x1d --proxy=${PROXY}
}

getTotalBorrowInEgld() {
    mxpy contract query ${ADDRESS} --function="getTotalBorrowInEgld" --arguments 0x1d --proxy=${PROXY}
}

getCollateralAmountForToken() {
    mxpy contract query ${ADDRESS} --function="getCollateralAmountForToken" --arguments 0x01 ${XEGLD_TOKEN} --proxy=${PROXY}
}

getBorrowAmountForToken() {
    mxpy contract query ${ADDRESS} --function="getBorrowAmountForToken" --arguments 0x06 ${USDC_TOKEN} --proxy=${PROXY}
}

getTotalCollateralAvailable() {
    mxpy contract query ${ADDRESS} --function="getTotalCollateralAvailable" --arguments 0x01 --proxy=${PROXY}
}

canBeLiquidated() {
    mxpy contract query ${ADDRESS} --function="canBeLiquidated" --arguments 0x06 --proxy=${PROXY}
}

getMaxLiquidateAmountForCollateral() {
    mxpy contract query ${ADDRESS} --function="getMaxLiquidateAmountForCollateral" --arguments 0x06 ${HTM_TOKEN} 0x01 --proxy=${PROXY}
}

getHealthFactor() {
    mxpy contract query ${ADDRESS} --function="getHealthFactor" --arguments 0x06 --proxy=${PROXY}
}

getTokenPriceData() {
    mxpy contract query ${ADDRESS} --function="getTokenPriceData" --arguments ${EGLD_TOKEN} --proxy=${PROXY}
}

getTokenPriceUSD() {
    mxpy contract query ${ADDRESS} --function="getTokenPriceUSD" --arguments ${LXOXNO_TOKEN} --proxy=${PROXY}
}

getTokenPriceEGLD() {
    mxpy contract query ${ADDRESS} --function="getTokenPriceEGLD" --arguments ${LXOXNO_TOKEN} --proxy=${PROXY}
}
