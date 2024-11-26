use lending_pool::*;
use multiversx_sc::{
    imports::StorageTokenWrapper,
    types::{
        EgldOrEsdtTokenIdentifier, EsdtLocalRole, EsdtTokenPayment, ManagedAddress, ManagedBuffer,
        ManagedVec, MultiEsdtPayment, MultiValueEncoded, ReturnsNewManagedAddress, ReturnsResult,
        TestEsdtTransfer, TestTokenIdentifier,
    },
};
use multiversx_sc_scenario::{
    api::StaticApi,
    imports::{BigUint, DebugApi, OptionalValue, ScenarioWorld, TestAddress, WhiteboxContract},
    rust_biguint, ScenarioTxRun, ScenarioTxWhitebox,
};

pub static NFT_ROLES: &[EsdtLocalRole] = &[
    EsdtLocalRole::NftCreate,
    EsdtLocalRole::Mint,
    EsdtLocalRole::NftBurn,
];
use std::ops::Mul;
pub mod constants;
mod proxy_aggregator;
mod proxy_lending_pool;
mod proxy_liquidity_pool;
pub mod setup;
pub const SECONDS_PER_YEAR: u64 = 31_556_926;

use constants::*;
use setup::{setup_lending_pool, setup_price_aggregator, setup_template_liquidity_pool};

fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new();

    blockchain.register_contract(LENDING_POOL_PATH, lending_pool::ContractBuilder);
    blockchain.register_contract(LIQUIDITY_POOL_PATH, liquidity_pool::ContractBuilder);
    blockchain.register_contract(PRICE_AGGREGATOR_PATH, price_aggregator::ContractBuilder);

    blockchain
}

struct LendingPoolTestState {
    world: ScenarioWorld,
    lending_pool_whitebox: WhiteboxContract<lending_pool::ContractObj<DebugApi>>,
    liquidity_pool_whitebox: WhiteboxContract<liquidity_pool::ContractObj<DebugApi>>,
    price_aggregator_whitebox: WhiteboxContract<price_aggregator::ContractObj<DebugApi>>,
    lending_sc: ManagedAddress<StaticApi>,
    template_address_liquidity_pool: ManagedAddress<StaticApi>,
    price_aggregator_sc: ManagedAddress<StaticApi>,
    usdc_market: ManagedAddress<StaticApi>,
    egld_market: ManagedAddress<StaticApi>,
    isolated_market: ManagedAddress<StaticApi>,
    siloed_market: ManagedAddress<StaticApi>,
    capped_market: ManagedAddress<StaticApi>,
}

impl LendingPoolTestState {
    fn new() -> Self {
        let mut world = world();

        world.account(OWNER_ADDRESS).nonce(1);
        world.current_block().block_timestamp(1);

        let (template_address_liquidity_pool, liquidity_pool_whitebox) =
            setup_template_liquidity_pool(&mut world);

        let (price_aggregator_sc, price_aggregator_whitebox) = setup_price_aggregator(&mut world);

        let (
            lending_sc,
            lending_pool_whitebox,
            usdc_market,
            egld_market,
            isolated_market,
            siloed_market,
            capped_market,
        ) = setup_lending_pool(
            &mut world,
            &template_address_liquidity_pool,
            &price_aggregator_sc,
        );

        Self {
            world,
            lending_pool_whitebox,
            liquidity_pool_whitebox,
            price_aggregator_whitebox,
            lending_sc,
            price_aggregator_sc,
            template_address_liquidity_pool,
            usdc_market,
            egld_market,
            isolated_market,
            siloed_market,
            capped_market,
        }
    }

    fn add_new_market(
        &mut self,
        token_id: TestTokenIdentifier,
        config: AssetConfig<StaticApi>,
        r_max: u64,
        r_base: u64,
        r_slope1: u64,
        r_slope2: u64,
        u_optimal: u64,
        reserve_factor: u64,
    ) -> ManagedAddress<StaticApi> {
        let market_address = self
            .world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .create_liquidity_pool(
                EgldOrEsdtTokenIdentifier::esdt(token_id.to_token_identifier()),
                r_max,
                r_base,
                r_slope1,
                r_slope2,
                u_optimal,
                reserve_factor,
                config,
            )
            .returns(ReturnsResult)
            .run();

        market_address
    }

    // Core lending operations
    fn supply_asset(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        decimals: u64,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

        if let OptionalValue::Some(account_nonce) = account_nonce {
            vec.push(EsdtTokenPayment::new(
                ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                BigUint::from(1u64),
            ));
        }
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(decimals as u32));
        vec.push(EsdtTokenPayment::new(
            token_id.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .supply(e_mode_category)
            .multi_esdt(vec)
            .run();
    }

    // Withdraw asset
    fn withdraw_asset(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
    ) {
        let transfer = EsdtTokenPayment::new(
            ACCOUNT_TOKEN.to_token_identifier(),
            account_nonce,
            BigUint::from(1u64),
        );

        let amount_to_withdraw = amount.mul(BigUint::from(10u64).pow(decimals as u32));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .withdraw(token_id.to_token_identifier(), amount_to_withdraw)
            .esdt(transfer)
            .run();
    }

    fn borrow_asset(
        &mut self,
        from: &TestAddress,
        asset_to_borrow: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .borrow(
                asset_to_borrow,
                amount * BigUint::from(10u64.pow(decimals as u32)),
            )
            .esdt(TestEsdtTransfer(ACCOUNT_TOKEN, account_nonce, 1u64))
            .run();
    }

    fn repay_asset(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
    ) {
        let amount_to_repay = amount.mul(BigUint::from(10u64).pow(decimals as u32));

        let transfer = EsdtTokenPayment::new(token.to_token_identifier(), 0, amount_to_repay);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .repay(account_nonce)
            .esdt(transfer)
            .run();
    }

    fn liquidate_account(
        &mut self,
        from: &TestAddress,
        collateral_to_liquidate: &TestTokenIdentifier,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(decimals as u32));
        let transfer = EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        );

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .liquidate(account_nonce, collateral_to_liquidate.to_token_identifier())
            .esdt(transfer)
            .run();
    }

    // Price aggregator operations
    fn submit_price(
        &mut self,
        price_aggregator_sc: &ManagedAddress<StaticApi>,
        from: &[u8],
        price: u64,
        decimals: u64,
        timestamp: u64,
    ) -> () {
        let oracles = vec![
            ORACLE_ADDRESS_1,
            ORACLE_ADDRESS_2,
            ORACLE_ADDRESS_3,
            ORACLE_ADDRESS_4,
        ];
        self.world
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
            self.world
                .tx()
                .from(oracle)
                .to(price_aggregator_sc)
                .typed(proxy_aggregator::PriceAggregatorProxy)
                .submit(
                    ManagedBuffer::from(from),
                    ManagedBuffer::from(DOLLAR_TICKER),
                    timestamp,
                    BigUint::from(price).mul(BigUint::from(BP)),
                    decimals as u8,
                )
                .run();
        }
    }

    fn get_market_utilization(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
        let utilization_ratio = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_capital_utilisation()
            .returns(ReturnsResult)
            .run();

        (utilization_ratio.to_u64().unwrap() as f64 * 100.0) / BP as f64
    }

    fn get_market_borrow_rate(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
        let borrow_rate = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_borrow_rate()
            .returns(ReturnsResult)
            .run();

        (borrow_rate.to_u64().unwrap() as f64 * 100.0) / BP as f64
    }

    fn get_market_supply_rate(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
        let supply_rate = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_deposit_rate()
            .returns(ReturnsResult)
            .run();

        (supply_rate.to_u64().unwrap() as f64 * 100.0) / BP as f64
    }

    fn get_market_total_capital(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> BigUint<StaticApi> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_total_capital()
            .returns(ReturnsResult)
            .run()
    }

    fn update_borrows_with_debt(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .update_borrows_with_debt(account_position)
            .run();
    }

    fn update_interest_indexes(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .update_collateral_with_interest(account_position)
            .run();
    }

    // View functions
    fn get_collateral_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> BigUint<StaticApi> {
        let collateral_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .get_collateral_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run();

        collateral_amount / BigUint::from(BP)
    }

    fn get_borrow_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> BigUint<StaticApi> {
        let token_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .get_borrow_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run();

        token_amount / BigUint::from(BP)
    }

    fn get_total_borrow_in_dollars(&mut self, account_position: u64) -> BigUint<StaticApi> {
        let borrow_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .get_total_borrow_in_dollars(account_position)
            .returns(ReturnsResult)
            .run();

        borrow_amount / BigUint::from(BP)
    }

    fn get_total_collateral_in_dollars(&mut self, account_position: u64) -> BigUint<StaticApi> {
        let collateral_amount_usd = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .get_total_collateral_in_dollars(account_position)
            .returns(ReturnsResult)
            .run();

        collateral_amount_usd / BigUint::from(BP)
    }
}

#[test]
fn test_basic_supply_and_borrow() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup accounts
    state.world.current_block().block_timestamp(0);
    state
        .world
        .account(supplier)
        .nonce(1)
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        );

    state.world.account(borrower).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Test supply
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    // Test borrow
    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Verify amounts
    let borrowed = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let collateral = state.get_collateral_amount_for_token(2, USDC_TOKEN);

    assert!(borrowed > BigUint::zero());
    assert!(collateral > BigUint::zero());
}

#[test]
fn test_interest_accrual() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");

    // Setup initial state
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);

    // Initial supply and borrow
    state.supply_asset(
        &supplier,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    state.supply_asset(
        &borrower,
        USDC_TOKEN,
        BigUint::from(10000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    state.borrow_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(50u64),
        2,
        EGLD_DECIMALS,
    );

    // Record initial amounts
    let initial_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let initial_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);
    const SECONDS_PER_DAY: u64 = 86_400; // 24 * 60 * 60

    // Simulate daily updates for a month
    for day in 1..=30 {
        state
            .world
            .current_block()
            .block_timestamp(SECONDS_PER_DAY * day);
        state.update_borrows_with_debt(&borrower, 2);
        state.update_interest_indexes(&supplier, 1);
    }

    // Verify interest accrual
    let final_borrow = state.get_borrow_amount_for_token(2, EGLD_TOKEN);
    let final_supply = state.get_collateral_amount_for_token(1, EGLD_TOKEN);

    assert!(final_borrow > initial_borrow);
    assert!(final_supply > initial_supply);
}

#[test]
fn test_liquidation() {
    let mut state = LendingPoolTestState::new();
    let supplier = TestAddress::new("supplier");
    let borrower = TestAddress::new("borrower");
    let liquidator = TestAddress::new("liquidator");

    // Setup accounts including liquidator
    state.world.current_block().block_timestamp(0);
    setup_accounts(&mut state, supplier, borrower);
    state.world.account(liquidator).nonce(1).esdt_balance(
        USDC_TOKEN,
        BigUint::from(20000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );

    // Create risky position
    state.supply_asset(
        &supplier,
        USDC_TOKEN,
        BigUint::from(5000u64),
        USDC_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    state.supply_asset(
        &borrower,
        EGLD_TOKEN,
        BigUint::from(100u64),
        EGLD_DECIMALS,
        OptionalValue::None,
        OptionalValue::None,
    );

    state.borrow_asset(
        &borrower,
        USDC_TOKEN.clone(),
        BigUint::from(2000u64),
        2,
        USDC_DECIMALS,
    );

    state.world.current_block().block_timestamp(1);

    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let collateral_in_dollars = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    println!("borrow_amount_in_dollars: {}", borrow_amount_in_dollars.to_u64().unwrap());
    println!(
        "collateral_in_dollars: {}",
        collateral_in_dollars.to_u64().unwrap()
    );
    state.world.current_block().block_timestamp(600000000u64);
    // state.submit_price(
    //     &state.price_aggregator_sc.clone(),
    //     EGLD_TOKEN.as_bytes(),
    //     EGLD_PRICE_IN_DOLLARS / 4,
    //     EGLD_DECIMALS,
    //     600000000u64,
    // );

    // Attempt liquidation
    state.liquidate_account(
        &liquidator,
        &EGLD_TOKEN,
        &USDC_TOKEN,
        BigUint::from(2500u64),
        2,
        USDC_DECIMALS,
    );

    let borrow_amount_in_dollars = state.get_borrow_amount_for_token(2, USDC_TOKEN);
    let collateral_in_dollars = state.get_collateral_amount_for_token(2, EGLD_TOKEN);

    println!("borrow_amount_in_dollars: {}", borrow_amount_in_dollars.to_u64().unwrap());
    println!(
        "collateral_in_dollars: {}",
        collateral_in_dollars.to_u64().unwrap()
    );
}

// Helper function for account setup
fn setup_accounts(state: &mut LendingPoolTestState, supplier: TestAddress, borrower: TestAddress) {
    state
        .world
        .account(supplier)
        .nonce(1)
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );

    state
        .world
        .account(borrower)
        .nonce(1)
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );
}
