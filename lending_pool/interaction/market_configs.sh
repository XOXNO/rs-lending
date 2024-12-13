#!/bin/bash

# Define market configurations as associative arrays
declare -A XEGLD_MARKET=(
    [token_id]="XEGLD-23b511"
    [ltv]="750000000000000000000"              # 80%
    [liquidation_threshold]="780000000000000000000"  # 85%
    [liquidation_bonus]="55000000000000000000"  # 5%
    [borrow_cap]="20000000000000000000000000"  # 1 WEGLD
    [supply_cap]="20000000000000000000000000" # 10 WEGLD
    [base_rate]="35000000000000000000"         # 1%
    [slope1]="250000000000000000000"          # 20%
    [slope2]="800000000000000000000"          # 50%
    [optimal_utilization]="920000000000000000000" # 80%
    [reserve_factor]="250000000000000000000" # 10%
    [can_be_collateral]="0x01"
    [can_be_borrowed]="0x01"
    [is_isolated]="0x00"
    [debt_ceiling_usd]="0x00"
    [flash_loan_fee]="5000000000000000000" # 0.5%
    [is_siloed]="0x00"
    [flashloan_enabled]="0x01"
    [can_borrow_in_isolation]="0x00"
    [oracle_address]="erd1qqqqqqqqqqqqqpgqhe8t5jewej4q8dzxeyj4edqg49cqpwc4pys2n8v9w"
    [oracle_method]="Mix"
    [oracle_type]="Normal"
    [oracle_source]="XExchange"
    [oracle_decimals]="18"
)

declare -A USDC_MARKET=(
    [token_id]="USDC-abcdef"
    [ltv]="8500"
    [liquidation_threshold]="9000"
    [liquidation_bonus]="500"
    [borrow_cap]="1000000"    # 1 USDC
    [supply_cap]="10000000"   # 10 USDC
    [base_rate]="50"          # 0.5%
    [slope1]="1000"           # 10%
    [slope2]="4000"           # 40%
    [optimal_utilization]="9000" # 90%
    [oracle_first_token]="USDC-abcdef"
    [oracle_second_token]=""
    [oracle_address]="erd1qqqqqqqqqqqqqpgqhe8t5jewej4q8dzxeyj4edqg49cqpwc4pys2n8v9w"
    [oracle_method]="Aggregator"
    [oracle_type]="Normal"
    [oracle_source]="None"
    [oracle_decimals]="6"
)

# Add more market configurations as needed

# Function to get market configuration
get_market_config() {
    local market_name=$1
    local var_name="${market_name}_MARKET"
    declare -n market_config=$var_name
    echo "${market_config[@]}"
}

# Function to create market
create_market() {
    local market_name=$1
    local var_name="${market_name}_MARKET"
    declare -n market_config=$var_name

    # Call the mxpy contract call with the market configuration
    mxpy --verbose contract call ${ADDRESS} \
        --proxy=${PROXY} \
        --chain=${CHAIN_ID} \
        --recall-nonce \
        --gas-limit=600000000 \
        --function="createMarket" \
        --arguments \
            ${market_config[token_id]} \
            ${market_config[ltv]} \
            ${market_config[liquidation_threshold]} \
            ${market_config[liquidation_bonus]} \
            ${market_config[borrow_cap]} \
            ${market_config[supply_cap]} \
            ${market_config[base_rate]} \
            ${market_config[slope1]} \
            ${market_config[slope2]} \
            ${market_config[optimal_utilization]} \
            ${market_config[oracle_first_token]} \
            ${market_config[oracle_second_token]} \
            ${market_config[oracle_address]} \
            ${market_config[oracle_method]} \
            ${market_config[oracle_type]} \
            ${market_config[oracle_source]} \
            ${market_config[oracle_decimals]} \
        --send \
        --pem=${WALLET}
}

# Function to list available markets
list_markets() {
    echo "Available markets:"
    echo "- WEGLD"
    echo "- USDC"
    # Add more markets as they are configured
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
    "list")
        list_markets
        ;;
    "show")
        if [ -z "$2" ]; then
            echo "Please specify a market name"
            list_markets
            exit 1
        fi
        get_market_config "$2"
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