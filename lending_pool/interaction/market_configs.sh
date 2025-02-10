#!/bin/bash

# Environment variables
ADDRESS=${ADDRESS:-"erd1qqqqqqqqqqqqqpgqg532f5mdsqganmfd5vwktt7a7rdnc2lgah0s9q46jx"}
AGGREGATOR_ADDRESS=${AGGREGATOR_ADDRESS:-"erd1qqqqqqqqqqqqqpgq7a48t570jjudy0xjxhuzcdwndcq9gt2tah0s7tg84a"}
CEX_AGGREGATOR_ADDRESS=${CEX_AGGREGATOR_ADDRESS:-"erd1qqqqqqqqqqqqqpgqlee5g4zqwq93ar9nlx55ql0jxvlrruadah0sg2vc89"}
PROXY=${PROXY:-"https://devnet-gateway.xoxno.com"}
CHAIN_ID=${CHAIN_ID:-"D"}

# Check if jq is installed
if ! command -v jq &> /dev/null; then
    echo "Error: jq is required but not installed."
    echo "Please install jq first:"
    echo "  macOS: brew install jq"
    echo "  Linux: sudo apt-get install jq"
    exit 1
fi

CONFIG_FILE="market_configs.json"

# Function to get config value
get_config_value() {
    local market=$1
    local field=$2
    jq -r ".[\"$market\"][\"$field\"]" "$CONFIG_FILE"
}

# Function to list available markets
list_markets() {
    echo "Available markets:"
    jq -r 'keys[]' "$CONFIG_FILE" | sed 's/^/- /'
}

upgrade_all_markets() {
    # Read all market names (keys) from the configuration file into an array
    local markets
    IFS=$'\n' read -d '' -r -a markets < <(jq -r 'keys[]' "$CONFIG_FILE" && printf '\0')
    
    for market in "${markets[@]}"; do
        echo "Upgrading market: $market"
        upgrade_market "$market"
        # Optionally wait a few seconds to ensure that the tx is processed before sending the next one
        sleep 5
    done
}

# Function to build market arguments
build_market_args() {
    local market_name=$1
    local -a args=()
    
    # Token configuration
    args+=("str:$(get_config_value "$market_name" "token_id")")

    # Interest rate parameters
    args+=("$(get_config_value "$market_name" "max_rate")")
    args+=("$(get_config_value "$market_name" "base_rate")")
    args+=("$(get_config_value "$market_name" "slope1")")
    args+=("$(get_config_value "$market_name" "slope2")")
    args+=("$(get_config_value "$market_name" "optimal_utilization")")
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

    # Caps
    args+=("$(get_config_value "$market_name" "borrow_cap")")
    args+=("$(get_config_value "$market_name" "supply_cap")")
    
    
    echo "${args[@]}"
}

# Function to build market arguments 
build_market_upgrade_args() {
    local market_name=$1
    local -a args=()
    
    # Token configuration
    args+=("str:$(get_config_value "$market_name" "token_id")")

    # Interest rate parameters
    args+=("$(get_config_value "$market_name" "max_rate")")
    args+=("$(get_config_value "$market_name" "base_rate")")
    args+=("$(get_config_value "$market_name" "slope1")")
    args+=("$(get_config_value "$market_name" "slope2")")
    args+=("$(get_config_value "$market_name" "optimal_utilization")")
    args+=("$(get_config_value "$market_name" "reserve_factor")")

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
    echo "${args[@]}"
}

create_set_decimals_aggregator_args() {
    local market_name=$1
    local -a args=()

    args+=("str:$market_name")
    args+=("str:USD")
    args+=("$(get_config_value "$market_name" "oracle_decimals")")
    echo "${args[@]}"
}

set_aggregator_decimals() {
    local market_name=$1
    
    echo "Creating token oracle for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(create_set_decimals_aggregator_args "$market_name") )
    echo "${args[@]}"

    mxpy contract call ${AGGREGATOR_ADDRESS} --recall-nonce --gas-limit=10000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setPairDecimals" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to create token oracle
create_token_oracle() {
    local market_name=$1
    
    echo "Creating token oracle for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(create_oracle_args "$market_name") )
    echo "${args[@]}"
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=20000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="setTokenOracle" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

upgrade_market() {
     local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_upgrade_args "$market_name") )

    mxpy contract call ${ADDRESS} --recall-nonce \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --gas-limit=50000000 \
    --function="upgradeLiquidityPool" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send || return
}

# Function to create market
create_market() {
    local market_name=$1
    
    echo "Creating market for ${market_name}..."
    echo "Token ID: $(get_config_value "$market_name" "token_id")"
    
    local args=( $(build_market_args "$market_name") )
    
    mxpy contract call ${ADDRESS} --recall-nonce --gas-limit=100000000 \
    --ledger --ledger-account-index=0 --ledger-address-index=0 \
    --function="createLiquidityPool" --arguments "${args[@]}" \
    --proxy=${PROXY} --chain=${CHAIN_ID} --send
}

# Function to format percentage
format_percentage() {
    local value=$1
    # Calculate percentage with high precision
    local result=$(echo "scale=3; $value/10^21 * 100" | bc)
    
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
    local decimals=$2
    local result=$(echo "scale=4; $value/10^$decimals" | bc)
    # Remove trailing zeros after decimal point
    echo $result | sed 's/\.0\+$\|0\+$//'
}

# Function to show market configuration
show_market_config() {
    local market=$1
    local decimals=$(get_config_value "$market" "oracle_decimals")
    
    echo "${market} Market Configuration:"
    echo "Token ID: $(get_config_value "$market" "token_id")"
    echo "LTV: $(format_percentage $(get_config_value "$market" "ltv"))%"
    echo "Liquidation Threshold: $(format_percentage $(get_config_value "$market" "liquidation_threshold"))%"
    echo "Liquidation Bonus: $(format_percentage $(get_config_value "$market" "liquidation_bonus"))%"
    echo "Liquidation Base Fee: $(format_percentage $(get_config_value "$market" "liquidation_base_fee"))%"
    echo "Borrow Cap: $(format_token_amount $(get_config_value "$market" "borrow_cap") $decimals) ${market}"
    echo "Supply Cap: $(format_token_amount $(get_config_value "$market" "supply_cap") $decimals) ${market}"
    echo "Base Rate: $(format_percentage $(get_config_value "$market" "base_rate"))%"
    echo "Max Rate: $(format_percentage $(get_config_value "$market" "max_rate"))%"
    echo "Slope1: $(format_percentage $(get_config_value "$market" "slope1"))%"
    echo "Slope2: $(format_percentage $(get_config_value "$market" "slope2"))%"
    echo "Optimal Utilization: $(format_percentage $(get_config_value "$market" "optimal_utilization"))%"
    echo "Reserve Factor: $(format_percentage $(get_config_value "$market" "reserve_factor"))%"
    echo "Can Be Collateral: $(get_config_value "$market" "can_be_collateral")"
    echo "Can Be Borrowed: $(get_config_value "$market" "can_be_borrowed")"
    echo "Is Isolated: $(get_config_value "$market" "is_isolated")"
    echo "Debt Ceiling: $(format_token_amount $(get_config_value "$market" "debt_ceiling_usd") 21) USD"
    echo "Flash Loan Fee: $(format_percentage $(get_config_value "$market" "flash_loan_fee"))%"
    echo "Is Siloed: $(get_config_value "$market" "is_siloed")"
    echo "Flashloan Enabled: $(get_config_value "$market" "flashloan_enabled")"
    echo "Can Borrow In Isolation: $(get_config_value "$market" "can_borrow_in_isolation")"
    echo "Oracle Address: $(get_config_value "$market" "oracle_address")"
    echo "Oracle Method: $(get_config_value "$market" "oracle_method")"
    echo "Oracle Type: $(get_config_value "$market" "oracle_type")"
    echo "Oracle Source: $(get_config_value "$market" "oracle_source")"
    echo "Oracle Decimals: $decimals"
}

# Main CLI interface
case "$1" in
    "create")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        create_market "$2"
        ;;
    "setDecimals")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        set_aggregator_decimals "$2"
        ;;
    "upgrade_market")
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
    "create_oracle")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        create_token_oracle "$2"
        ;; 
    "list")
        list_markets
        ;;
    "show")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        show_market_config "$2"
        ;;
    *)
        echo "Usage: $0 {create|list|show} [market_name]"
        echo "Commands:"
        echo "  create MARKET  - Create a new market with specified configuration"
        echo "  list          - List available market configurations"
        echo "  show MARKET   - Show configuration for specified market"
        exit 1
        ;;
esac