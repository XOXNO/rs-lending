ADDRESS=erd1qqqqqqqqqqqqqpgq7a48t570jjudy0xjxhuzcdwndcq9gt2tah0s7tg84a #main
# ADDRESS=erd1qqqqqqqqqqqqqpgqlee5g4zqwq93ar9nlx55ql0jxvlrruadah0sg2vc89 #CEX

PROXY=https://devnet-gateway.xoxno.com
CHAIN_ID=D

PROJECT="./output/price_aggregator.wasm"

deploy() {
    mxpy --verbose contract deploy --bytecode=${PROJECT} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=250000000 --outfile="deploy.json" --arguments 0x01 erd1e2r4m874a7whe5n2sftrxqy758kl3s6gvs8ekmac2yuhxlg70shs5lhc7t erd13r5dhrpx8wk6l4lvwr52y5thx7q56g4w9l7sznef3vlp8yqvqmfs5l2vq0 erd1l2u03tmtpphrwwq0xn3gfu23ufzjexu0p54zya8ufhlyrrx64vzsvyfgvg erd12yxd5phejzw83gn8qh6jfz6q9a0ekyyhkfd3c49r03mxw25l3a5swq3nf7 erd1nz0w0cnlpxsdqa2vm6mfmzc9qhptjae9x44kn3fkzsgns8dcz5pscja98t erd16yaq7n30gdka6hvnly365kecckqqsl48m2g8vrrmcr5w6h79pussyc4wu2 erd14xglrv7xt8pu90zgx0v9dd5u8cu4crl6yw7x0vfgse4r3dkkqnuse59u75 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

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

addOracles() {
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=30000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="addOracles" --arguments erd1e2r4m874a7whe5n2sftrxqy758kl3s6gvs8ekmac2yuhxlg70shs5lhc7t erd13r5dhrpx8wk6l4lvwr52y5thx7q56g4w9l7sznef3vlp8yqvqmfs5l2vq0 erd1l2u03tmtpphrwwq0xn3gfu23ufzjexu0p54zya8ufhlyrrx64vzsvyfgvg erd12yxd5phejzw83gn8qh6jfz6q9a0ekyyhkfd3c49r03mxw25l3a5swq3nf7 erd1nz0w0cnlpxsdqa2vm6mfmzc9qhptjae9x44kn3fkzsgns8dcz5pscja98t erd16yaq7n30gdka6hvnly365kecckqqsl48m2g8vrrmcr5w6h79pussyc4wu2 erd14xglrv7xt8pu90zgx0v9dd5u8cu4crl6yw7x0vfgse4r3dkkqnuse59u75 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

FROM="str:USDT"
TO="str:USD"
PRICE=1511388968601690100000 # 55 USD

DECIMALS=6

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
    --function="submit" --arguments ${FROM} ${TO} ${timestamp} ${PRICE} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
    }

getPairDecimals() {
    mxpy contract query ${ADDRESS} --function="getPairDecimals" --arguments ${FROM} ${TO} --proxy=${PROXY}
}