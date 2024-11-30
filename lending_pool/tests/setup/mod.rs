use crate::{constants::*, proxys::*};
use lending_pool::{AccountTokenModule, EModeAssetConfig, EModeCategory, BP};
use multiversx_sc::{
    imports::{OptionalValue, StorageTokenWrapper},
    types::{
        BigUint, ManagedAddress, ManagedBuffer, MultiValueEncoded, ReturnsNewManagedAddress,
        ReturnsResult, TestTokenIdentifier,
    },
};
use multiversx_sc_scenario::{
    api::StaticApi, DebugApi, ScenarioTxRun, ScenarioTxWhitebox, ScenarioWorld, WhiteboxContract,
};
use std::ops::Mul;

use lending_pool::*;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EsdtLocalRole, EsdtTokenPayment, ManagedVec, TestEsdtTransfer,
};
use multiversx_sc_scenario::imports::{ExpectMessage, TestAddress};

pub static NFT_ROLES: &[EsdtLocalRole] = &[
    EsdtLocalRole::NftCreate,
    EsdtLocalRole::Mint,
    EsdtLocalRole::NftBurn,
];

pub fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new();

    blockchain.register_contract(LENDING_POOL_PATH, lending_pool::ContractBuilder);
    blockchain.register_contract(LIQUIDITY_POOL_PATH, liquidity_pool::ContractBuilder);
    blockchain.register_contract(PRICE_AGGREGATOR_PATH, price_aggregator::ContractBuilder);

    blockchain
}

pub struct LendingPoolTestState {
    pub world: ScenarioWorld,
    pub lending_pool_whitebox: WhiteboxContract<lending_pool::ContractObj<DebugApi>>,
    pub liquidity_pool_whitebox: WhiteboxContract<liquidity_pool::ContractObj<DebugApi>>,
    pub price_aggregator_whitebox: WhiteboxContract<price_aggregator::ContractObj<DebugApi>>,
    pub lending_sc: ManagedAddress<StaticApi>,
    pub template_address_liquidity_pool: ManagedAddress<StaticApi>,
    pub price_aggregator_sc: ManagedAddress<StaticApi>,
    pub usdc_market: ManagedAddress<StaticApi>,
    pub egld_market: ManagedAddress<StaticApi>,
    pub isolated_market: ManagedAddress<StaticApi>,
    pub siloed_market: ManagedAddress<StaticApi>,
    pub capped_market: ManagedAddress<StaticApi>,
    pub xegld_market: ManagedAddress<StaticApi>,
    pub segld_market: ManagedAddress<StaticApi>,
    pub legld_market: ManagedAddress<StaticApi>,
}

impl LendingPoolTestState {
    pub fn new() -> Self {
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
            xegld_market,
            segld_market,
            legld_market,
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
            xegld_market,
            segld_market,
            legld_market,
        }
    }

    pub fn add_new_market(
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
                config.ltv,
                config.liquidation_threshold,
                config.liquidation_bonus,
                config.liquidation_base_fee,
                config.can_be_collateral,
                config.can_be_borrowed,
                config.is_isolated,
                config.debt_ceiling_usd,
                config.flash_loan_fee,
                config.is_siloed,
                config.flashloan_enabled,
                config.can_borrow_in_isolation,
                OptionalValue::from(config.borrow_cap),
                OptionalValue::from(config.supply_cap),
            )
            .returns(ReturnsResult)
            .run();

        market_address
    }

    // Core lending operations
    pub fn supply_asset(
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

    // Core lending operations
    pub fn supply_asset_error(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        decimals: u64,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        error_message: &[u8],
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
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn supply_asset_error_payment_count(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        decimals: u64,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        error_message: &[u8],
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
            amount_to_transfer.clone(),
        ));
        vec.push(EsdtTokenPayment::new(
            token_id.to_token_identifier(),
            0,
            amount_to_transfer.clone(),
        ));

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
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    // Withdraw asset
    pub fn withdraw_asset(
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

    pub fn withdraw_asset_error(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
        error_message: &[u8],
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
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn borrow_asset(
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

    pub fn borrow_asset_error(
        &mut self,
        from: &TestAddress,
        asset_to_borrow: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        decimals: u64,
        error_message: &[u8],
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
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn repay_asset(
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

    pub fn liquidate_account(
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
    pub fn submit_price(
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

    pub fn get_market_utilization(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
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

    pub fn get_market_borrow_rate(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
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

    pub fn get_market_supply_rate(&mut self, market_address: ManagedAddress<StaticApi>) -> f64 {
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

    pub fn get_market_total_capital(
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

    pub fn update_borrows_with_debt(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .update_borrows_with_debt(account_position)
            .run();
    }

    pub fn update_interest_indexes(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::LendingPoolProxy)
            .update_collateral_with_interest(account_position)
            .run();
    }

    // View functions
    pub fn get_collateral_amount_for_token(
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

        collateral_amount
    }

    pub fn get_borrow_amount_for_token(
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

        token_amount
    }

    pub fn get_total_borrow_in_dollars(&mut self, account_position: u64) -> BigUint<StaticApi> {
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

    pub fn get_total_collateral_in_dollars(&mut self, account_position: u64) -> BigUint<StaticApi> {
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
    let xegld_market = setup_market(world, &lending_sc, XEGLD_TOKEN, get_xegld_config());
    let isolated_market = setup_market(world, &lending_sc, ISOLATED_TOKEN, get_isolated_config());
    let siloed_market = setup_market(world, &lending_sc, SILOED_TOKEN, get_siloed_config());
    let capped_market = setup_market(world, &lending_sc, CAPPED_TOKEN, get_capped_config());
    let segld_market = setup_market(world, &lending_sc, SEGLD_TOKEN, get_segld_config());
    let legld_market = setup_market(world, &lending_sc, LEGLD_TOKEN, get_legld_config());

    create_e_mode_category(world, &lending_sc);

    add_asset_to_e_mode_category(world, &lending_sc, EGLD_TOKEN, true, true, 1);
    add_asset_to_e_mode_category(world, &lending_sc, XEGLD_TOKEN, true, true, 1);
    add_asset_to_e_mode_category(world, &lending_sc, SEGLD_TOKEN, false, true, 1);
    add_asset_to_e_mode_category(world, &lending_sc, LEGLD_TOKEN, false, false, 1);
    (
        lending_sc,
        lending_pool_whitebox,
        usdc_market,
        egld_market,
        isolated_market,
        siloed_market,
        capped_market,
        xegld_market,
        segld_market,
        legld_market,
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
pub fn create_e_mode_category(world: &mut ScenarioWorld, lending_sc: &ManagedAddress<StaticApi>) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::LendingPoolProxy)
        .add_e_mode_category(EModeCategory {
            id: 1,
            ltv: BigUint::from(LTV),
            liquidation_threshold: BigUint::from(E_MODE_LIQ_THRESOLD),
            liquidation_bonus: BigUint::from(E_MODE_LIQ_BONUS),
        })
        .returns(ReturnsResult)
        .run();
}
pub fn add_asset_to_e_mode_category(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    asset: TestTokenIdentifier,
    can_be_collateral: bool,
    can_be_borrowed: bool,
    category_id: u8,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::LendingPoolProxy)
        .add_asset_to_e_mode_category(
            asset,
            category_id,
            EModeAssetConfig {
                can_be_collateral,
                can_be_borrowed,
            },
        )
        .returns(ReturnsResult)
        .run();
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
            config.config.ltv,
            config.config.liquidation_threshold,
            config.config.liquidation_bonus,
            config.config.liquidation_base_fee,
            config.config.can_be_collateral,
            config.config.can_be_borrowed,
            config.config.is_isolated,
            config.config.debt_ceiling_usd,
            config.config.flash_loan_fee,
            config.config.is_siloed,
            config.config.flashloan_enabled,
            config.config.can_borrow_in_isolation,
            OptionalValue::from(config.config.borrow_cap),
            OptionalValue::from(config.config.supply_cap),
        )
        .returns(ReturnsResult)
        .run();

    market_address
}


// Helper function for account setup
pub fn setup_accounts(state: &mut LendingPoolTestState, supplier: TestAddress, borrower: TestAddress) {
    state
        .world
        .account(supplier)
        .nonce(1)
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            ISOLATED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32),
        )
        .esdt_balance(
            SILOED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SILOED_DECIMALS as u32),
        )
        .esdt_balance(
            XEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(XEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            SEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
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
            ISOLATED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32),
        )
        .esdt_balance(
            SILOED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SILOED_DECIMALS as u32),
        )
        .esdt_balance(
            XEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            SEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );
}
