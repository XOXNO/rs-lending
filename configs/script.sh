#!/bin/bash

# Default network
NETWORK=${NETWORK:-"devnet"}

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed."
    echo "Please install jq first:"
    echo "  macOS: brew install jq"
    echo "  Linux: sudo apt-get install jq"
    exit 1
fi

NETWORKS_FILE="networks.json"
MARKET_CONFIG_FILE="configs/${NETWORK}_market_configs.json"
EMODES_CONFIG_FILE="configs/emodes.json"

# Contract paths configuration
OUTPUT_DOCKER="output-docker"
CONTROLLER_NAME="controller"
MARKET_NAME="liquidity_layer"
PRICE_AGGREGATOR_NAME="price_aggregator"

# WASM paths
PROJECT_CONTROLLER="./${OUTPUT_DOCKER}/${CONTROLLER_NAME}/${CONTROLLER_NAME}.wasm"
PROJECT_MARKET="./${OUTPUT_DOCKER}/${MARKET_NAME}/${MARKET_NAME}.wasm"
PRICE_AGGREGATOR_PATH="./${OUTPUT_DOCKER}/${PRICE_AGGREGATOR_NAME}/${PRICE_AGGREGATOR_NAME}.wasm"

# Source JSON paths for contract verification
CONTROLLER_SOURCE="${OUTPUT_DOCKER}/${CONTROLLER_NAME}/${CONTROLLER_NAME}-0.0.0.source.json"
MARKET_SOURCE="${OUTPUT_DOCKER}/${MARKET_NAME}/${MARKET_NAME}-0.0.1.source.json"
PRICE_AGGREGATOR_SOURCE="${OUTPUT_DOCKER}/${PRICE_AGGREGATOR_NAME}/${PRICE_AGGREGATOR_NAME}-0.57.0.source.json"

# Verification configuration
if [ "$NETWORK" = "devnet" ]; then
    VERIFIER_URL="https://devnet-play-api.multiversx.com"
else
    VERIFIER_URL="https://play-api.multiversx.com"
fi
DOCKER_IMAGE="multiversx/sdk-rust-contract-builder:v9.0.0"

# Check if market config file exists
if [ ! -f "$MARKET_CONFIG_FILE" ]; then
    echo "Error: Market configuration file not found: $MARKET_CONFIG_FILE"
    exit 1
fi

# Check if emodes config file exists
if [ ! -f "$EMODES_CONFIG_FILE" ]; then
    echo "Error: E-Mode configuration file not found: $EMODES_CONFIG_FILE"
    exit 1
fi

# Load network configuration
if [ ! -f "$NETWORKS_FILE" ]; then
    echo "Error: Network configuration file not found: $NETWORKS_FILE"
    exit 1
fi

# Function to verify contract
verifyContract() {
    local contract_address=$1
    local source_json=$2
    local contract_name=$3

    echo "Verifying ${contract_name} contract on ${NETWORK}..."
    echo "Contract address: ${contract_address}"
    echo "Source JSON: ${source_json}"

    mxpy contract verify "${contract_address}" \
    --packaged-src="${source_json}" \
    --verifier-url="${VERIFIER_URL}" \
    --docker-image="${DOCKER_IMAGE}" \
    --ledger \
    --ledger-account-index=${LEDGER_ACCOUNT_INDEX} \
    --ledger-address-index=${LEDGER_ADDRESS_INDEX} || return
}

# Function to verify specific contracts
verifyControllerContract() {
    verifyContract "${ADDRESS}" "${CONTROLLER_SOURCE}" "controller"
}

verifyMarketContract() {
    local market_address=$1
    if [ -z "$market_address" ]; then
        echo "Error: Market address is required for verification"
        echo "Usage: verifyMarket <market_address>"
        exit 1
    fi
    verifyContract "$market_address" "${MARKET_SOURCE}" "market"
}

verifyPriceAggregatorContract() {
    verifyContract "${PRICE_AGGREGATOR_ADDRESS}" "${PRICE_AGGREGATOR_SOURCE}" "price_aggregator"
}

# Function to get network configuration value
get_network_value() {
    local path=$1
    jq -r ".$NETWORK.$path" "$NETWORKS_FILE"
}

# Function to get config value from the market configs file
get_config_value() {
    local market=$1
    local field=$2
    jq -r ".[\"$market\"][\"$field\"]" "$MARKET_CONFIG_FILE"
}

# Function to get emode config value from the emodes config file
get_emode_config_value() {
    local category_id=$1
    local path=$2
    jq -r ".\"$NETWORK\".\"$category_id\"$path" "$EMODES_CONFIG_FILE"
}

# Load network-specific configurations
PROXY=$(get_network_value "proxy")
CHAIN_ID=$(get_network_value "chain_id")

# Load ledger configuration
LEDGER_ACCOUNT_INDEX=$(get_network_value "ledger.account_index")
LEDGER_ADDRESS_INDEX=$(get_network_value "ledger.address_index")

# Load addresses
ADDRESS=$(get_network_value "addresses.controller")
LP_TEMPLATE_ADDRESS=$(get_network_value "addresses.lp_template") 
ASH_ADDRESS=$(get_network_value "addresses.ash")
WEGLD_ADDRESS=$(get_network_value "addresses.wegld")
SAFE_PRICE_VIEW_ADDRESS=$(get_network_value "addresses.safe_price_view")
ACCUMULATOR_ADDRESS=$(get_network_value "addresses.accumulator")
PRICE_AGGREGATOR_ADDRESS=$(get_network_value "addresses.price_aggregator")

# Load price aggregator config
PRICE_AGGREGATOR_ORACLES=$(get_network_value "oracles[]" | tr '\n' ' ')

# Load account token config
ACCOUNT_TOKEN_NAME="str:$(get_network_value "account_token.name")"
ACCOUNT_TOKEN_TICKER="str:$(get_network_value "account_token.ticker")"
ISSUE_COST=$(get_network_value "account_token.issue_cost")


echo "Using network: $NETWORK"
echo "Proxy: $PROXY"
echo "Chain ID: $CHAIN_ID"
echo "Controller address: $ADDRESS"

# Function to list available markets
list_markets() {
    echo "Available markets in $MARKET_CONFIG_FILE:"
    jq -r 'keys[]' "$MARKET_CONFIG_FILE" | sed 's/^/- /'
}

# Function to upgrade all markets
upgrade_all_markets() {
    # Read all market names (keys) from the configuration file into an array
    local markets
    IFS=$'\n' read -d '' -r -a markets < <(jq -r 'keys[]' "$MARKET_CONFIG_FILE" && printf '\0')
    
    for market in "${markets[@]}"; do
        echo "Upgrading market: $market"
        upgrade_market "$market"
        # Optionally wait a few seconds to ensure that the tx is processed before sending the next one
        sleep 5
    done
}

# Function to convert percentage to RAY (27 decimals)
to_ray() {
    local value=$1
    local numeric_value=$(echo "$value" | sed 's/[^0-9.]//g')
    
    if [ -z "$numeric_value" ] || [ "$numeric_value" = "null" ]; then
        echo "0"
        return
    fi
    
    # Use higher precision for the division, then set scale=0 only for the final result
    # This ensures values like 40/100 = 0.4 are preserved during calculation
    local result=$(echo "scale=10; temp = ($numeric_value / 100) * 1000000000000000000000000000; scale=0; temp / 1" | bc)
    
    # If result is empty (bc error), return 0
    if [ -z "$result" ]; then
        echo "0"
    else
        echo "$result"
    fi
}

# Function to convert a number to the correct decimal places based on oracle_decimals
to_decimals() {
    local value=$1
    local decimals=$2
    echo "${value}$(printf '%0*d' "$decimals" 0)"
}

# Function to build market arguments
build_market_args() {
    local market_name=$1
    local -a args=()
    local oracle_decimals=$(get_config_value "$market_name" "oracle_decimals")
    
    # Debug output
    # echo "Building market args for $market_name:"
    # echo "Token ID: $(get_config_value "$market_name" "token_id")"
    # echo "Max rate: $(get_config_value "$market_name" "max_rate")"
    # echo "Base rate: $(get_config_value "$market_name" "base_rate")"
    # echo "Slope1: $(get_config_value "$market_name" "slope1")"
    # echo "Slope2: $(get_config_value "$market_name" "slope2")"
    # echo "Slope3: $(get_config_value "$market_name" "slope3")"
    # echo "Mid utilization: $(get_config_value "$market_name" "mid_utilization")"
    # echo "Optimal utilization: $(get_config_value "$market_name" "optimal_utilization")"
    
    # Token configuration
    args+=("str:$(get_config_value "$market_name" "token_id")")

    # Interest rate parameters - convert from percentage to RAY
    max_rate=$(to_ray "$(get_config_value "$market_name" "max_rate")")
    base_rate=$(to_ray "$(get_config_value "$market_name" "base_rate")")
    slope1=$(to_ray "$(get_config_value "$market_name" "slope1")")
    slope2=$(to_ray "$(get_config_value "$market_name" "slope2")")
    slope3=$(to_ray "$(get_config_value "$market_name" "slope3")")
    mid_util=$(to_ray "$(get_config_value "$market_name" "mid_utilization")")
    opt_util=$(to_ray "$(get_config_value "$market_name" "optimal_utilization")")
    
    # echo "Converted values:"
    # echo "Max rate (RAY): $max_rate"
    # echo "Base rate (RAY): $base_rate"
    # echo "Slope1 (RAY): $slope1"
    # echo "Slope2 (RAY): $slope2"
    # echo "Slope3 (RAY): $slope3"
    # echo "Mid utilization (RAY): $mid_util"
    # echo "Optimal utilization (RAY): $opt_util"
    
    args+=("$max_rate")
    args+=("$base_rate")
    args+=("$slope1")
    args+=("$slope2")
    args+=("$slope3")
    args+=("$mid_util") 
    args+=("$opt_util")
    args+=("$(get_config_value "$market_name" "reserve_factor")")

    # Risk parameters
    args+=("$(get_config_value "$market_name" "ltv")")
    args+=("$(get_config_value "$market_name" "liquidation_threshold")")
    args+=("$(get_config_value "$market_name" "liquidation_bonus")")
    args+=("$(get_config_value "$market_name" "liquidation_base_fee")")
    
    # Flags
    args+=("$(get_config_value "$market_name" "can_be_collateral")")
    args+=("$(get_config_value "$market_name" "can_be_borrowed")")
    args+=("$(get_config_value "$market_name" "is_isolated")")
    args+=("$(get_config_value "$market_name" "debt_ceiling_usd")")
    args+=("$(get_config_value "$market_name" "flash_loan_fee")")
    args+=("$(get_config_value "$market_name" "is_siloed")")
    args+=("$(get_config_value "$market_name" "flashloan_enabled")")
    args+=("$(get_config_value "$market_name" "can_borrow_in_isolation")")
    args+=("$(get_config_value "$market_name" "oracle_decimals")")

    # Caps - scale according to oracle_decimals
    args+=("$(to_decimals "$(get_config_value "$market_name" "borrow_cap")" "$oracle_decimals")")
    args+=("$(to_decimals "$(get_config_value "$market_name" "supply_cap")" "$oracle_decimals")")
    
    echo "${args[@]}"
}

# Function to build market upgrade arguments 
build_market_upgrade_args() {
    local market_name=$1
    local -a args=()
    
    # Token configuration
    args+=("str:$(get_config_value "$market_name" "token_id")")

    # Interest rate parameters - convert from percentage to RAY
    args+=("$(to_ray "$(get_config_value "$market_name" "max_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "base_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope1")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope2")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope3")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "mid_utilization")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "optimal_utilization")")")
    args+=("$(get_config_value "$market_name" "reserve_factor")")

    echo "${args[@]}"
}

# Function to build market template arguments
build_market_template_upgrade_args() {
    local market_name=$1
    local -a args=()

    # Interest rate parameters - convert from percentage to RAY
    args+=("$(to_ray "$(get_config_value "$market_name" "max_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "base_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope1")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope2")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope3")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "mid_utilization")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "optimal_utilization")")")
    args+=("$(get_config_value "$market_name" "reserve_factor")")

    echo "${args[@]}"
}

build_market_template_deploy_args() {
    local market_name=$1
    local -a args=()
    
    # Token configuration
    args+=("str:$(get_config_value "$market_name" "token_id")")

    # Interest rate parameters - convert from percentage to RAY
    args+=("$(to_ray "$(get_config_value "$market_name" "max_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "base_rate")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope1")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope2")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "slope3")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "mid_utilization")")")
    args+=("$(to_ray "$(get_config_value "$market_name" "optimal_utilization")")")
    args+=("$(get_config_value "$market_name" "reserve_factor")")
    args+=("$(get_config_value "$market_name" "oracle_decimals")")

    echo "${args[@]}"
}

create_oracle_args() {
    local market_name=$1
    local -a args=()

    args+=("str:$(get_config_value "$market_name" "token_id")")
    args+=("$(get_config_value "$market_name" "oracle_decimals")")
    args+=("$(get_config_value "$market_name" "oracle_address")")
    args+=("$(get_config_value "$market_name" "oracle_method")")
    args+=("$(get_config_value "$market_name" "oracle_type")")
    args+=("$(get_config_value "$market_name" "oracle_source")")
    args+=("$(get_config_value "$market_name" "first_tolerance")")
    args+=("$(get_config_value "$market_name" "last_tolerance")")
    
    # Check if pair_id exists in config and add it if present
    local pair_id=$(jq -r ".[\"$market_name\"][\"pair_id\"] // empty" "$MARKET_CONFIG_FILE")
    if [ ! -z "$pair_id" ]; then
        args+=("$pair_id")
    fi
    
    echo "${args[@]}"
}

# Price Aggregator Functions
deploy_price_aggregator() {
    echo "Deploying price aggregator for network: $NETWORK"
    echo "Contract path: $PRICE_AGGREGATOR_PATH"

    # Get submission counts from network configuration
    local submission_counts=$(get_network_value "submission_counts")
    echo "Using submission counts: $submission_counts"

    # Convert oracle addresses to CLI arguments
    read -a oracle_array <<< "$PRICE_AGGREGATOR_ORACLES"

    mxpy contract deploy --bytecode=${PRICE_AGGREGATOR_PATH} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=250000000 --outfile="deploy-price-aggregator-${NETWORK}.json" --arguments 0x$(printf "%02x" $submission_counts) ${oracle_array[@]} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return

    echo ""
    echo "Price aggregator contract address: ${PRICE_AGGREGATOR_ADDRESS}"
}

upgrade_price_aggregator() {
    echo "Upgrading price aggregator for network: $NETWORK"
    echo "Contract address: $PRICE_AGGREGATOR_ADDRESS"
    
    mxpy contract upgrade ${PRICE_AGGREGATOR_ADDRESS} --bytecode=${PRICE_AGGREGATOR_PATH} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=100000000 --outfile="upgrade-price-aggregator-${NETWORK}.json" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

unpause_price_aggregator() {
    echo "Unpausing price aggregator for network: $NETWORK"
    echo "Contract address: $PRICE_AGGREGATOR_ADDRESS"
    
    mxpy contract call ${PRICE_AGGREGATOR_ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="unpause" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

pause_price_aggregator() {
    echo "Pausing price aggregator for network: $NETWORK"
    echo "Contract address: $PRICE_AGGREGATOR_ADDRESS"
    
    mxpy contract call ${PRICE_AGGREGATOR_ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="pause" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

add_oracles_price_aggregator() {
    echo "Adding oracles to price aggregator for network: $NETWORK"
    echo "Contract address: $PRICE_AGGREGATOR_ADDRESS"
    
    # Get oracle addresses from function arguments
    local -a oracle_addresses=("$@")
    
    if [ ${#oracle_addresses[@]} -eq 0 ]; then
        echo "Error: No oracle addresses provided."
        echo "Usage: addOracles <address1> <address2> ..."
        exit 1
    fi
    
    echo "Adding ${#oracle_addresses[@]} oracles: ${oracle_addresses[*]}"
    
    mxpy contract call ${PRICE_AGGREGATOR_ADDRESS} --recall-nonce --gas-limit=30000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="addOracles" --arguments "${oracle_addresses[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

deploy_controller() {
    mxpy --verbose contract deploy --bytecode=${PROJECT_CONTROLLER} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=450000000 --outfile="deploy-${NETWORK}.json" --arguments ${LP_TEMPLATE_ADDRESS} ${PRICE_AGGREGATOR_ADDRESS} ${SAFE_PRICE_VIEW_ADDRESS} ${ACCUMULATOR_ADDRESS} ${WEGLD_ADDRESS} ${ASH_ADDRESS} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

upgrade_controller() {
    mxpy contract upgrade ${ADDRESS} --bytecode=${PROJECT_CONTROLLER} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=550000000 \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

deploy_market_template() {
    local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_template_deploy_args "$market_name") )

    echo "${args[@]}"

    mxpy contract deploy --bytecode=${PROJECT_MARKET} \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --recall-nonce --gas-limit=250000000 \
    --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

upgrade_market_template() {
    local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_template_upgrade_args "$market_name") )

    mxpy contract upgrade ${LP_TEMPLATE_ADDRESS} \
    --bytecode=${PROJECT_MARKET} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=250000000 \
    --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

# Function to create token oracle
create_token_oracle() {
    local market_name=$1
    
    echo "Creating token oracle for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(create_oracle_args "$market_name") )
    echo "${args[@]}"
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=100000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="setTokenOracle" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

upgrade_market() {
    local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_upgrade_args "$market_name") )

    mxpy contract call ${ADDRESS} --recall-nonce \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --gas-limit=50000000 \
    --function="upgradeLiquidityPool" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

registerAccountToken() {
    mxpy contract call ${ADDRESS} --recall-nonce  --gas-limit=100000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="registerAccountToken" --value=${ISSUE_COST} --arguments ${ACCOUNT_TOKEN_NAME} ${ACCOUNT_TOKEN_TICKER} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

# Function to create market
create_market() {
    local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_args "$market_name") )

    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=100000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="createLiquidityPool" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to format percentage
format_percentage() {
    local value=$1
    local decimals=${2:-4}  # Default to 4 decimals if not specified
    
    # Calculate percentage with high precision
    local result=$(echo "scale=3; $value/10^$decimals * 100" | bc)
    
    # If the number starts with a dot, add a leading zero
    if [[ $result == .* ]]; then
        result="0$result"
    fi
    
    # Remove trailing zeros after decimal point, but keep at least one decimal if it's a decimal number
    result=$(echo $result | sed 's/\.0*$\|0*$//')
    
    # If no decimal point in result, add .0
    if [[ $result != *.* ]]; then
        result="$result.0"
    fi
    
    echo $result
}

# Function to format token amount
format_token_amount() {
    local value=$1
    local asset_decimals=$2
    local result=$(echo "scale=4; $value/10^$asset_decimals" | bc)
    # Remove trailing zeros after decimal point
    echo $result | sed 's/\.0\+$\|0\+$//'
}

# Function to show market configuration
show_market_config() {
    local market=$1
    local asset_decimals=$(get_config_value "$market" "oracle_decimals")
    
    echo "${market} Market Configuration:"
    echo "Token ID: $(get_config_value "$market" "token_id")"
    echo "LTV: $(format_percentage $(get_config_value "$market" "ltv"))%"
    echo "Liquidation Threshold: $(format_percentage $(get_config_value "$market" "liquidation_threshold"))%"
    echo "Liquidation Bonus: $(format_percentage $(get_config_value "$market" "liquidation_bonus"))%"
    echo "Liquidation Base Fee: $(format_percentage $(get_config_value "$market" "liquidation_base_fee"))%"
    echo "Borrow Cap: $(get_config_value "$market" "borrow_cap") ${market}"
    echo "Supply Cap: $(get_config_value "$market" "supply_cap") ${market}"
    
    # Interest rate parameters - already in percentage format in config
    echo "Base Rate: $(get_config_value "$market" "base_rate")%"
    echo "Max Rate: $(get_config_value "$market" "max_rate")%"
    echo "Slope1: $(get_config_value "$market" "slope1")%"
    echo "Slope2: $(get_config_value "$market" "slope2")%"
    echo "Slope3: $(get_config_value "$market" "slope3")%"
    echo "Mid Utilization: $(get_config_value "$market" "mid_utilization")%"
    echo "Optimal Utilization: $(get_config_value "$market" "optimal_utilization")%"
    
    echo "Reserve Factor: $(format_percentage $(get_config_value "$market" "reserve_factor"))%"
    echo "Can Be Collateral: $(get_config_value "$market" "can_be_collateral")"
    echo "Can Be Borrowed: $(get_config_value "$market" "can_be_borrowed")"
    echo "Is Isolated: $(get_config_value "$market" "is_isolated")"
    echo "Debt Ceiling: $(format_token_amount $(get_config_value "$market" "debt_ceiling_usd") 18) USD"
    echo "Flash Loan Fee: $(format_percentage $(get_config_value "$market" "flash_loan_fee"))%"
    echo "Is Siloed: $(get_config_value "$market" "is_siloed")"
    echo "Flashloan Enabled: $(get_config_value "$market" "flashloan_enabled")"
    echo "Can Borrow In Isolation: $(get_config_value "$market" "can_borrow_in_isolation")"
    echo "Oracle Address: $(get_config_value "$market" "oracle_address")"
    echo "Oracle Method: $(get_config_value "$market" "oracle_method")"
    echo "Oracle Type: $(get_config_value "$market" "oracle_type")"
    echo "Oracle Source: $(get_config_value "$market" "oracle_source")"
    echo "Oracle Decimals: $asset_decimals"
}

# Print available networks
list_networks() {
    echo "Available networks:"
    jq -r 'keys[]' "$NETWORKS_FILE" | sed 's/^/- /'
}

# Function to edit token oracle tolerance
edit_token_oracle_tolerance() {
    local market_name=$1
    local -a args=()
    
    args+=("str:$(get_config_value "$market_name" "token_id")")
    args+=("$(get_config_value "$market_name" "first_tolerance")")
    args+=("$(get_config_value "$market_name" "last_tolerance")")
    
    echo "Editing token oracle tolerance for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    echo "First Tolerance: $(get_config_value "$market_name" "first_tolerance")"
    echo "Last Tolerance: $(get_config_value "$market_name" "last_tolerance")"
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="editTokenOracleTolerance" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to list available emode categories
list_emode_categories() {
    echo "Available E-Mode categories for $NETWORK:"
    jq -r ".$NETWORK | keys[]" "$EMODES_CONFIG_FILE" | while read -r category_id; do
        name=$(get_emode_config_value "$category_id" ".name")
        echo "- Category $category_id: $name"
        
        # List assets in this category
        jq -r ".$NETWORK.\"$category_id\".assets | keys[]" "$EMODES_CONFIG_FILE" | while read -r asset; do
            can_be_collateral=$(get_emode_config_value "$category_id" ".assets.\"$asset\".can_be_collateral")
            can_be_borrowed=$(get_emode_config_value "$category_id" ".assets.\"$asset\".can_be_borrowed")
            
            collateral_status="Not Collateral"
            if [ "$can_be_collateral" = "0x01" ]; then
                collateral_status="Collateral"
            fi
            
            borrow_status="Not Borrowable"
            if [ "$can_be_borrowed" = "0x01" ]; then
                borrow_status="Borrowable"
            fi
            
            echo "  - $asset ($collateral_status, $borrow_status)"
        done
    done
}

# Function to add E-Mode category
add_emode_category() {
    local category_id=$1
    
    echo "Adding E-Mode category ${category_id}..."
    echo "Name: $(get_emode_config_value "$category_id" ".name")"
    
    local ltv=$(get_emode_config_value "$category_id" ".ltv")
    local liquidation_threshold=$(get_emode_config_value "$category_id" ".liquidation_threshold")
    local liquidation_bonus=$(get_emode_config_value "$category_id" ".liquidation_bonus")
    
    echo "LTV: ${ltv}"
    echo "Liquidation Threshold: ${liquidation_threshold}"
    echo "Liquidation Bonus: ${liquidation_bonus}"
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="addEModeCategory" --arguments ${ltv} ${liquidation_threshold} ${liquidation_bonus} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to add asset to E-Mode category
add_asset_to_emode_category() {
    local category_id=$1
    local asset_name=$2
    
    echo "Adding asset ${asset_name} to E-Mode category ${category_id}..."
    
    local token_id=$(get_config_value "$asset_name" "token_id")
    local can_be_collateral=$(get_emode_config_value "$category_id" ".assets.\"$asset_name\".can_be_collateral")
    local can_be_borrowed=$(get_emode_config_value "$category_id" ".assets.\"$asset_name\".can_be_borrowed")
    
    echo "Token ID: ${token_id}"
    echo "Can Be Collateral: ${can_be_collateral}"
    echo "Can Be Borrowed: ${can_be_borrowed}"
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="addAssetToEModeCategory" --arguments "str:${token_id}" ${category_id} ${can_be_collateral} ${can_be_borrowed} \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to edit asset config
edit_asset_config() {
    local market_name=$1
    
    echo "Editing asset config for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local -a args=()
    
    # Token identifier
    args+=("str:$(get_config_value "$market_name" "token_id")")
    
    # Risk parameters
    args+=("$(get_config_value "$market_name" "ltv")")
    args+=("$(get_config_value "$market_name" "liquidation_threshold")")
    args+=("$(get_config_value "$market_name" "liquidation_bonus")")
    args+=("$(get_config_value "$market_name" "liquidation_base_fee")")
    
    # Flags
    args+=("$(get_config_value "$market_name" "is_isolated")")
    args+=("$(get_config_value "$market_name" "debt_ceiling_usd")")
    args+=("$(get_config_value "$market_name" "is_siloed")")
    args+=("$(get_config_value "$market_name" "flashloan_enabled")")
    args+=("$(get_config_value "$market_name" "flash_loan_fee")")
    args+=("$(get_config_value "$market_name" "can_be_collateral")")
    args+=("$(get_config_value "$market_name" "can_be_borrowed")")
    args+=("$(get_config_value "$market_name" "can_borrow_in_isolation")")
    
    # Caps
    args+=("$(get_config_value "$market_name" "borrow_cap")")
    args+=("$(get_config_value "$market_name" "supply_cap")")
    
    echo "${args[@]}"
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="editAssetConfig" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to claim revenue from all markets
claim_revenue() {
    echo "Claiming revenue from all markets..."
    
    # Get all token IDs from the config file
    local token_ids=($(jq -r 'to_entries[] | .value.token_id' "$MARKET_CONFIG_FILE"))
    
    if [ ${#token_ids[@]} -eq 0 ]; then
        echo "No markets found in configuration"
        exit 1
    fi
    
    echo "Token IDs to claim revenue from: ${token_ids[*]}"
    
    # Prepare arguments for the contract call
    local args=()
    for token_id in "${token_ids[@]}"; do
        args+=("str:$token_id")
    done
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=450000000 \
    --ledger --ledger-account-index=${LEDGER_ACCOUNT_INDEX} --ledger-address-index=${LEDGER_ADDRESS_INDEX} \
    --function="claimRevenue" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Main CLI interface
case "$1" in
    "deployMarketTemplate")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        deploy_market_template "$2"
        ;;
    "upgradeMarketTemplate")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        upgrade_market_template "$2"
        ;;
    "deployController")
        deploy_controller
        ;;
    "registerAccountToken")
        registerAccountToken
        ;;
    "claimRevenue")
        claim_revenue
        ;;
    "addOracles")
        shift  # Remove the first argument (command name)
        if [ $# -eq 0 ]; then
            echo "Please specify oracle addresses"
            exit 1
        fi
        add_oracles_price_aggregator "$@"
        ;;
    "createOracle")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        create_token_oracle "$2"
        ;; 
    "createMarket")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        create_market "$2"
        ;;
    "upgradeController")
        upgrade_controller
        ;;
    "setDecimals")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        set_aggregator_decimals "$2"
        ;;
    "upgradeMarket")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        upgrade_market "$2"
        ;;
    "upgradeAllMarkets")
        upgrade_all_markets
        ;;
    "deployPriceAggregator")
        deploy_price_aggregator
        ;;
    "upgradePriceAggregator")
        upgrade_price_aggregator
        ;;
    "pauseAggregator")
        pause_price_aggregator
        ;;
    "unpauseAggregator")
        unpause_price_aggregator
        ;;
    "editOracleTolerance")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        edit_token_oracle_tolerance "$2"
        ;;
    "addEModeCategory")
        if [ -z "$2" ]; then
            echo "Please specify a category ID"
            list_emode_categories
            exit 1
        fi
        add_emode_category "$2"
        ;;
    "addAssetToEMode")
        if [ -z "$2" ] || [ -z "$3" ]; then
            echo "Please specify a category ID and asset name"
            list_emode_categories
            exit 1
        fi
        add_asset_to_emode_category "$2" "$3"
        ;;
    "editAssetConfig")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        edit_asset_config "$2"
        ;;
    "listEModeCategories")
        list_emode_categories
        ;;
    "list")
        list_markets
        ;;
    "networks")
        list_networks
        ;;
    "show")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        show_market_config "$2"
        ;;
    "listMarkets")
        list_markets
        ;;
    "verifyController")
        verifyControllerContract
        ;;
    "verifyMarket")
        if [ -z "$2" ]; then
            echo "Please specify a market address"
            echo "Usage: verifyMarket <market_address>"
            exit 1
        fi
        verifyMarketContract "$2"
        ;;
    "verifyPriceAggregator")
        verifyPriceAggregatorContract
        ;;
    *)
        echo "Usage: $0 COMMAND [ARGS]"
        echo ""
        echo "Environment variables:"
        echo "  NETWORK - Specify network (devnet, mainnet), default: devnet"
        echo ""
        echo "Commands:"
        echo "  deployController               - Deploy a new controller contract"
        echo "  upgradeController              - Upgrade an existing controller contract"
        echo "  registerAccountToken           - Register a new account token for NFT positions"
        echo "  createMarket MARKET            - Create a new market with specified configuration"
        echo "  createOracle MARKET            - Create oracle for a market"
        echo "  upgradeMarket MARKET           - Upgrade an existing market"
        echo "  upgradeAllMarkets              - Upgrade all markets"
        echo "  listMarkets                    - List available market configurations"
        echo "  show MARKET                    - Show configuration for specified market"
        echo "  deployMarketTemplate MARKET    - Deploy a new market template"
        echo "  upgradeMarketTemplate MARKET   - Upgrade a market template"
        echo "  setDecimals MARKET             - Set decimals for market in price aggregator"
        echo "  networks                       - List available networks"
        echo "  claimRevenue                   - Claim revenue from all markets"
        echo ""
        echo "Price Aggregator Commands:"
        echo "  deployPriceAggregator         - Deploy the price aggregator contract"
        echo "  upgradePriceAggregator        - Upgrade the price aggregator contract"
        echo "  pauseAggregator               - Pause the price aggregator contract"
        echo "  unpauseAggregator             - Unpause the price aggregator contract"
        echo "  addOracles                    - Add oracles to the price aggregator contract"
        echo ""
        echo "E-Mode Commands:"
        echo "  editOracleTolerance MARKET    - Edit token oracle tolerance settings"
        echo "  addEModeCategory ID           - Add an E-Mode category"
        echo "  addAssetToEMode ID ASSET      - Add an asset to an E-Mode category"
        echo "  editAssetConfig MARKET        - Edit asset configuration"
        echo "  listEModeCategories           - List all E-Mode categories"
        echo ""
        echo "Verification Commands:"
        echo "  verifyController              - Verify the controller contract"
        echo "  verifyMarket                  - Verify the market contract"
        echo "  verifyPriceAggregator         - Verify the price aggregator contract"
        exit 1
        ;;
esac