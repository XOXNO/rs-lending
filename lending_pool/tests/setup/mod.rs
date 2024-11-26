use crate::{constants::*, proxy_aggregator, proxy_lending_pool, proxy_liquidity_pool, NFT_ROLES};
use lending_pool::{AccountTokenModule, BP};
use multiversx_sc::{
    imports::StorageTokenWrapper,
    types::{
        BigUint, ManagedAddress, ManagedBuffer, MultiValueEncoded, ReturnsNewManagedAddress,
        ReturnsResult, TestTokenIdentifier,
    },
};
use multiversx_sc_scenario::{
    api::StaticApi, DebugApi, ScenarioTxRun, ScenarioTxWhitebox, ScenarioWorld, WhiteboxContract,
};
use std::ops::Mul;
pub fn setup_lending_pool(
    world: &mut ScenarioWorld,
    template_address_liquidity_pool: &ManagedAddress<StaticApi>,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<lending_pool::ContractObj<DebugApi>>,
    ManagedAddress<StaticApi>,
    ManagedAddress<StaticApi>,
    ManagedAddress<StaticApi>,
    ManagedAddress<StaticApi>,
    ManagedAddress<StaticApi>,
) {
    let lending_pool_whitebox =
        WhiteboxContract::new(LENDING_POOL_ADDRESS, lending_pool::contract_obj);

    let lending_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_lending_pool::LendingPoolProxy)
        .init(template_address_liquidity_pool, price_aggregator_sc)
        .code(LENDING_POOL_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world.set_esdt_local_roles(lending_sc.clone(), ACCOUNT_TOKEN.as_bytes(), NFT_ROLES);

    // Set the token id for the account token
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc.clone())
        .whitebox(lending_pool::contract_obj, |sc| {
            sc.account_token()
                .set_token_id(ACCOUNT_TOKEN.to_token_identifier());
        });

    let usdc_market = setup_market(world, &lending_sc, USDC_TOKEN, get_usdc_config());
    let egld_market = setup_market(world, &lending_sc, EGLD_TOKEN, get_egld_config());
    let isolated_market = setup_market(world, &lending_sc, ISOLATED_TOKEN, get_isolated_config());
    let siloed_market = setup_market(world, &lending_sc, SILOED_TOKEN, get_siloed_config());
    let capped_market = setup_market(world, &lending_sc, CAPPED_TOKEN, get_capped_config());

    (
        lending_sc,
        lending_pool_whitebox,
        usdc_market,
        egld_market,
        isolated_market,
        siloed_market,
        capped_market,
    )
}

pub fn setup_price_aggregator(
    world: &mut ScenarioWorld,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<price_aggregator::ContractObj<DebugApi>>,
) {
    let price_aggregator_whitebox =
        WhiteboxContract::new(PRICE_AGGREGATOR_ADDRESS, price_aggregator::contract_obj);
    world.account(ORACLE_ADDRESS_1).nonce(1);
    world.account(ORACLE_ADDRESS_2).nonce(1);
    world.account(ORACLE_ADDRESS_3).nonce(1);
    world.account(ORACLE_ADDRESS_4).nonce(1);

    let mut oracles = MultiValueEncoded::new();
    oracles.push(ORACLE_ADDRESS_1.to_managed_address());
    oracles.push(ORACLE_ADDRESS_2.to_managed_address());
    oracles.push(ORACLE_ADDRESS_3.to_managed_address());
    oracles.push(ORACLE_ADDRESS_4.to_managed_address());

    let price_aggregator_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_aggregator::PriceAggregatorProxy)
        .init(4usize, oracles)
        .code(PRICE_AGGREGATOR_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(&price_aggregator_sc)
        .typed(proxy_aggregator::PriceAggregatorProxy)
        .unpause_endpoint()
        .run();

    submit_price(
        world,
        &price_aggregator_sc,
        EGLD_TICKER,
        EGLD_PRICE_IN_DOLLARS,
        EGLD_DECIMALS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        USDC_TICKER,
        USDC_PRICE_IN_DOLLARS,
        USDC_DECIMALS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        XEGLD_TICKER,
        XEGLD_PRICE_IN_DOLLARS,
        XEGLD_DECIMALS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        ISOLATED_TICKER,
        ISOLATED_PRICE_IN_DOLLARS,
        ISOLATED_DECIMALS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        SILOED_TICKER,
        SILOED_PRICE_IN_DOLLARS,
        SILOED_DECIMALS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        CAPPED_TICKER,
        CAPPED_PRICE_IN_DOLLARS,
        CAPPED_DECIMALS,
    );

    (price_aggregator_sc, price_aggregator_whitebox)
}

pub fn submit_price(
    world: &mut ScenarioWorld,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
    from: &[u8],
    price: u64,
    decimals: u64,
) -> () {
    let oracles = vec![
        ORACLE_ADDRESS_1,
        ORACLE_ADDRESS_2,
        ORACLE_ADDRESS_3,
        ORACLE_ADDRESS_4,
    ];

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(price_aggregator_sc)
        .typed(proxy_aggregator::PriceAggregatorProxy)
        .set_pair_decimals(
            ManagedBuffer::from(from),
            ManagedBuffer::from(DOLLAR_TICKER),
            decimals as u8,
        )
        .run();
    for oracle in oracles {
        world
            .tx()
            .from(oracle)
            .to(price_aggregator_sc)
            .typed(proxy_aggregator::PriceAggregatorProxy)
            .submit(
                ManagedBuffer::from(from),
                ManagedBuffer::from(DOLLAR_TICKER),
                1u64,
                BigUint::from(price).mul(BigUint::from(BP)),
                decimals as u8,
            )
            .run();
    }
}

pub fn setup_template_liquidity_pool(
    world: &mut ScenarioWorld,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<liquidity_pool::ContractObj<DebugApi>>,
) {
    let liquidity_pool_whitebox =
        WhiteboxContract::new(LIQUIDITY_POOL_ADDRESS, liquidity_pool::contract_obj);

    let template_address_liquidity_pool = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_liquidity_pool::LiquidityPoolProxy)
        .init(
            USDC_TICKER,
            R_MAX,
            R_BASE,
            R_SLOPE1,
            R_SLOPE2,
            U_OPTIMAL,
            RESERVE_FACTOR,
        )
        .code(LIQUIDITY_POOL_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    (template_address_liquidity_pool, liquidity_pool_whitebox)
}

pub fn setup_market(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    token: TestTokenIdentifier,
    config: SetupConfig,
) -> ManagedAddress<StaticApi> {
    let market_address = world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::LendingPoolProxy)
        .create_liquidity_pool(
            token.to_token_identifier(),
            config.r_max,
            config.r_base,
            config.r_slope1,
            config.r_slope2,
            config.u_optimal,
            config.reserve_factor,
            config.config,
        )
        .returns(ReturnsResult)
        .run();

    market_address
}
