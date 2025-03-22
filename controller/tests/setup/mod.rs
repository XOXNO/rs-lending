use crate::{constants::*, proxys::*};
use common_constants::{EGLD_TICKER, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE};

use cache::Cache;

use multiversx_sc::{
    imports::{MultiValue2, OptionalValue},
    types::{
        BigUint, EgldOrEsdtTokenPayment, ManagedAddress, ManagedArgBuffer, ManagedBuffer,
        ManagedDecimal, MultiValueEncoded, NumDecimals, ReturnsNewManagedAddress, ReturnsResult,
        TestTokenIdentifier,
    },
};
use multiversx_sc_scenario::{
    api::StaticApi, DebugApi, ScenarioTxRun, ScenarioTxWhitebox, ScenarioWorld, WhiteboxContract,
};
use pair::config::ConfigModule;
use rs_liquid_staking_sc::{
    proxy::proxy_liquid_staking::{self, ScoringConfig},
    storage::StorageModule,
};
use rs_liquid_xoxno::{config::ConfigModule as XoxnoConfigModule, rs_xoxno_proxy};

use std::ops::Mul;
use storage::Storage;

use controller::{positions::update::PositionUpdateModule, *};
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EsdtLocalRole, EsdtTokenPayment, ManagedVec, TestEsdtTransfer,
};
use multiversx_sc_scenario::imports::{ExpectMessage, TestAddress};

pub static NFT_ROLES: &[EsdtLocalRole] = &[
    EsdtLocalRole::NftCreate,
    EsdtLocalRole::Mint,
    EsdtLocalRole::NftBurn,
    EsdtLocalRole::NftUpdateAttributes,
];

pub fn world() -> ScenarioWorld {
    let mut blockchain = ScenarioWorld::new();

    blockchain.register_contract(LENDING_POOL_PATH, controller::ContractBuilder);
    blockchain.register_contract(LIQUIDITY_POOL_PATH, liquidity_layer::ContractBuilder);
    blockchain.register_contract(PRICE_AGGREGATOR_PATH, price_aggregator::ContractBuilder);
    blockchain.register_contract(
        EGLD_LIQUID_STAKING_PATH,
        rs_liquid_staking_sc::ContractBuilder,
    );
    blockchain.register_contract(XOXNO_LIQUID_STAKING_PATH, rs_liquid_xoxno::ContractBuilder);
    blockchain.register_contract(PAIR_PATH, pair::ContractBuilder);

    blockchain.register_contract(SAFE_PRICE_VIEW_PATH, pair::ContractBuilder);

    blockchain.register_contract(FLASH_MOCK_PATH, flash_mock::ContractBuilder);

    blockchain
}

pub struct LendingPoolTestState {
    pub world: ScenarioWorld,
    pub lending_pool_whitebox: WhiteboxContract<controller::ContractObj<DebugApi>>,
    pub liquidity_pool_whitebox: WhiteboxContract<liquidity_layer::ContractObj<DebugApi>>,
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
    pub lp_egld_market: ManagedAddress<StaticApi>,
    pub xoxno_market: ManagedAddress<StaticApi>,
    pub flash_mock: ManagedAddress<StaticApi>,
}

impl LendingPoolTestState {
    pub fn new() -> Self {
        let mut world = world();
        setup_owner(&mut world);
        world.current_block().block_timestamp(0);

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
            lp_egld_market,
            xoxno_market,
        ) = setup_lending_pool(
            &mut world,
            &template_address_liquidity_pool,
            &price_aggregator_sc,
        );

        let flash_mock = setup_flash_mock(&mut world);
        setup_flasher(&mut world, flash_mock.clone());
        // For LP safe price simulation
        world.current_block().block_round(1500);

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
            lp_egld_market,
            xoxno_market,
            flash_mock,
        }
    }

    // pub fn calculate_max_leverage(
    //     &mut self,
    //     initial_deposit: BigUint<StaticApi>,
    //     target_hf: BigUint<StaticApi>,
    //     reserves: BigUint<StaticApi>,
    //     reservers_factor: BigUint<StaticApi>,
    // ) -> BigUint<StaticApi> {
    //     // let max_liquidate_amount = self
    //     //     .world
    //     //     .query()
    //     //     .to(self.lending_sc.clone())
    //     //     .typed(proxy_lending_pool::ControllerProxy)
    //     //     .calculate_max_leverage(
    //     //         &initial_deposit,
    //     //         &target_hf,
    //     //         Option::<EModeCategory<StaticApi>>::None,
    //     //         get_usdc_config().config,
    //     //         &reserves,
    //     //         &reservers_factor,
    //     //     )
    //     //     .returns(ReturnsResult)
    //     //     .run();

    //     // println!("Max Leverage: {:?}", max_liquidate_amount);
    //     // return max_liquidate_amount;
    // }

    pub fn edit_asset_config(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        loan_to_value: &BigUint<StaticApi>,
        liquidation_threshold: &BigUint<StaticApi>,
        liquidation_bonus: &BigUint<StaticApi>,
        liquidation_fees: &BigUint<StaticApi>,
        is_isolated_asset: bool,
        isolation_debt_ceiling_usd: &BigUint<StaticApi>,
        is_siloed_borrowing: bool,
        is_flashloanable: bool,
        flashloan_fee: &BigUint<StaticApi>,
        is_collateralizable: bool,
        is_borrowable: bool,
        isolation_borrow_enabled: bool,
        borrow_cap: &BigUint<StaticApi>,
        supply_cap: &BigUint<StaticApi>,
        error_message: Option<&[u8]>,
    ) {
        let call = self
            .world
            .tx()
            .from(OWNER_ADDRESS.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_asset_config(
                asset,
                loan_to_value,
                liquidation_threshold,
                liquidation_bonus,
                liquidation_fees,
                is_isolated_asset,
                isolation_debt_ceiling_usd,
                is_siloed_borrowing,
                is_flashloanable,
                flashloan_fee,
                is_collateralizable,
                is_borrowable,
                isolation_borrow_enabled,
                borrow_cap,
                supply_cap,
            );
        if error_message.is_some() {
            call.returns(ExpectMessage(
                core::str::from_utf8(error_message.unwrap()).unwrap(),
            ))
            .run();
        } else {
            call.run();
        }
    }

    pub fn get_usd_price(
        &mut self,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_usd_price(token_id)
            .returns(ReturnsResult)
            .run()
    }

    pub fn get_egld_price(
        &mut self,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_egld_price(token_id)
            .returns(ReturnsResult)
            .run()
    }

    pub fn flash_loan(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        contract: ManagedAddress<StaticApi>,
        endpoint: ManagedBuffer<StaticApi>,
        arguments: ManagedArgBuffer<StaticApi>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .flash_loan(token, amount, contract, endpoint, arguments)
            .run();
    }

    pub fn flash_loan_error(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        contract: ManagedAddress<StaticApi>,
        endpoint: ManagedBuffer<StaticApi>,
        arguments: ManagedArgBuffer<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .flash_loan(token, amount, contract, endpoint, arguments)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn enable_vault(&mut self, from: &TestAddress, account_nonce: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .toggle_vault(true)
            .single_esdt(
                &ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                &BigUint::from(1u32),
            )
            .run();
    }

    pub fn enable_vault_error(
        &mut self,
        from: &TestAddress,
        account_nonce: u64,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .toggle_vault(true)
            .single_esdt(
                &ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                &BigUint::from(1u32),
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn disable_vault(&mut self, from: &TestAddress, account_nonce: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .toggle_vault(false)
            .single_esdt(
                &ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                &BigUint::from(1u32),
            )
            .run();
    }

    pub fn disable_vault_error(
        &mut self,
        from: &TestAddress,
        account_nonce: u64,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .toggle_vault(false)
            .single_esdt(
                &ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                &BigUint::from(1u32),
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn get_usd_price_error(&mut self, token_id: TestTokenIdentifier, error_message: &[u8]) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_usd_price(token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }
    pub fn add_new_market(
        &mut self,
        token_id: EgldOrEsdtTokenIdentifier<StaticApi>,
        config: AssetConfig<StaticApi>,
        max_borrow_rate: u64,
        base_borrow_rate: u64,
        slope1: u64,
        slope2: u64,
        slope3: u64,
        mid_utilization: u64,
        optimal_utilization: u64,
        reserve_factor: u64,
        asset_decimals: usize,
    ) -> ManagedAddress<StaticApi> {
        let market_address = self
            .world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .create_liquidity_pool(
                token_id,
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
                config.loan_to_value.into_raw_units(),
                config.liquidation_threshold.into_raw_units(),
                config.liquidation_bonus.into_raw_units(),
                config.liquidation_fees.into_raw_units(),
                config.is_collateralizable,
                config.is_borrowable,
                config.is_isolated_asset,
                config.isolation_debt_ceiling_usd.into_raw_units(),
                config.flashloan_fee.into_raw_units(),
                config.is_siloed_borrowing,
                config.is_flashloanable,
                config.isolation_borrow_enabled,
                asset_decimals,
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
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

        if let OptionalValue::Some(account_nonce) = account_nonce {
            vec.push(EsdtTokenPayment::new(
                ACCOUNT_TOKEN.to_token_identifier(),
                account_nonce,
                BigUint::from(1u64),
            ));
        }
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        vec.push(EsdtTokenPayment::new(
            token_id.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .multi_esdt(vec)
            .run();
    }

    // Core lending operations
    pub fn supply_asset_error(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
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
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        vec.push(EsdtTokenPayment::new(
            token_id.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    // Core lending operations
    pub fn supply_empty_asset_error(
        &mut self,
        from: &TestAddress,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
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

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn supply_bulk_error(
        &mut self,
        from: &TestAddress,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
        assets: ManagedVec<StaticApi, EsdtTokenPayment<StaticApi>>,
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
        vec.extend(assets);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn empty_supply_asset_error(
        &mut self,
        from: &TestAddress,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn supply_asset_error_payment_count(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        is_vault: bool,
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
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
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
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(is_vault, e_mode_category)
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn claim_revenue(&mut self, token_id: TestTokenIdentifier) {
        let mut array = MultiValueEncoded::new();
        array.push(EgldOrEsdtTokenIdentifier::esdt(
            token_id.to_token_identifier(),
        ));
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .claim_revenue(array)
            .run();
    }

    // Withdraw asset
    pub fn withdraw_asset(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
    ) {
        let transfer = EsdtTokenPayment::new(
            ACCOUNT_TOKEN.to_token_identifier(),
            account_nonce,
            BigUint::from(1u64),
        );

        let amount_to_withdraw = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(token_id.to_token_identifier()),
            0,
            amount_to_withdraw,
        );
        let mut array: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
            MultiValueEncoded::new();
        array.push(asset);
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .withdraw(array)
            .esdt(transfer)
            .run();
    }

    pub fn withdraw_asset_den(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let transfer = EsdtTokenPayment::new(
            ACCOUNT_TOKEN.to_token_identifier(),
            account_nonce,
            BigUint::from(1u64),
        );

        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(token_id.to_token_identifier()),
            0,
            amount,
        );
        let mut array: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
            MultiValueEncoded::new();
        array.push(asset);
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .withdraw(array)
            .esdt(transfer)
            .run();
    }

    pub fn withdraw_asset_error(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
        error_message: &[u8],
    ) {
        let transfer = EsdtTokenPayment::new(
            ACCOUNT_TOKEN.to_token_identifier(),
            account_nonce,
            BigUint::from(1u64),
        );

        let amount_to_withdraw = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(token_id.to_token_identifier()),
            0,
            amount_to_withdraw,
        );
        let mut array: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
            MultiValueEncoded::new();
        array.push(asset);
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .withdraw(array)
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
        asset_decimals: usize,
    ) {
        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(asset_to_borrow.to_token_identifier()),
            0,
            amount * BigUint::from(10u64.pow(asset_decimals as u32)),
        );
        let mut array: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
            MultiValueEncoded::new();
        array.push(asset);
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow(array)
            .esdt(TestEsdtTransfer(ACCOUNT_TOKEN, account_nonce, 1u64))
            .run();
    }

    pub fn borrow_assets(
        &mut self,
        account_nonce: u64,
        from: &TestAddress,
        assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow(assets)
            .esdt(TestEsdtTransfer(ACCOUNT_TOKEN, account_nonce, 1u64))
            .run();
    }

    pub fn borrow_asset_error(
        &mut self,
        from: &TestAddress,
        asset_to_borrow: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
        error_message: &[u8],
    ) {
        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(asset_to_borrow.to_token_identifier()),
            0,
            amount * BigUint::from(10u64.pow(asset_decimals as u32)),
        );
        let mut array: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>> =
            MultiValueEncoded::new();
        array.push(asset);
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow(array)
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
        asset_decimals: usize,
    ) {
        let amount_to_repay = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));

        let transfer = EsdtTokenPayment::new(token.to_token_identifier(), 0, amount_to_repay);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .repay(account_nonce)
            .esdt(transfer)
            .run();
    }

    pub fn repay_asset_deno(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let transfer = EsdtTokenPayment::new(token.to_token_identifier(), 0, amount);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .repay(account_nonce)
            .esdt(transfer)
            .run();
    }

    pub fn liquidate_account(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        let transfer = EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        );

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .esdt(transfer)
            .run();
    }

    pub fn liquidate_account_den(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount_to_transfer: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let transfer = EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        );

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .esdt(transfer)
            .run();
    }

    pub fn liquidate_account_dem(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let transfer = EsdtTokenPayment::new(liquidator_payment.to_token_identifier(), 0, amount);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .esdt(transfer)
            .run();
    }

    // Price aggregator operations
    pub fn submit_price(&mut self, from: &[u8], price: u64, timestamp: u64) -> () {
        let oracles = vec![
            ORACLE_ADDRESS_1,
            ORACLE_ADDRESS_2,
            ORACLE_ADDRESS_3,
            ORACLE_ADDRESS_4,
        ];
        for oracle in oracles {
            self.world
                .tx()
                .from(oracle)
                .to(self.price_aggregator_sc.clone())
                .typed(proxy_aggregator::PriceAggregatorProxy)
                .submit(
                    ManagedBuffer::from(from),
                    ManagedBuffer::from(DOLLAR_TICKER),
                    timestamp,
                    BigUint::from(price).mul(BigUint::from(WAD)),
                )
                .run();
        }
    }

    // Price aggregator operations
    pub fn submit_price_denom(
        &mut self,
        from: &[u8],
        price: BigUint<StaticApi>,
        timestamp: u64,
    ) -> () {
        let oracles = vec![
            ORACLE_ADDRESS_1,
            ORACLE_ADDRESS_2,
            ORACLE_ADDRESS_3,
            ORACLE_ADDRESS_4,
        ];
        for oracle in oracles {
            self.world
                .tx()
                .from(oracle)
                .to(self.price_aggregator_sc.clone())
                .typed(proxy_aggregator::PriceAggregatorProxy)
                .submit(
                    ManagedBuffer::from(from),
                    ManagedBuffer::from(DOLLAR_TICKER),
                    timestamp,
                    &price,
                )
                .run();
        }
    }

    pub fn get_market_utilization(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, usize> {
        let utilization_ratio = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_capital_utilisation()
            .returns(ReturnsResult)
            .run();

        utilization_ratio
    }

    pub fn get_market_revenue(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let revenue = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .revenue()
            .returns(ReturnsResult)
            .run();

        revenue
    }

    pub fn get_market_borrow_index(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let borrow_index = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .borrow_index()
            .returns(ReturnsResult)
            .run();

        borrow_index
    }

    pub fn get_market_supply_index(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let supply_index = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .supply_index()
            .returns(ReturnsResult)
            .run();

        supply_index
    }

    pub fn get_market_reserves(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let reserves = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .reserves()
            .returns(ReturnsResult)
            .run();

        reserves
    }
    pub fn get_market_borrow_rate(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, usize> {
        let borrow_rate = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_borrow_rate()
            .returns(ReturnsResult)
            .run();

        borrow_rate
    }
    pub fn get_account_health_factor(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let health_factor = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_health_factor(account_position)
            .returns(ReturnsResult)
            .run();

        health_factor
    }

    pub fn can_be_liquidated(&mut self, account_position: u64) -> bool {
        let can_be_liquidated = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .can_be_liquidated(account_position)
            .returns(ReturnsResult)
            .run();

        can_be_liquidated
    }
    pub fn get_market_supply_rate(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, usize> {
        let supply_rate = self
            .world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .get_deposit_rate()
            .returns(ReturnsResult)
            .run();

        supply_rate
    }

    pub fn get_market_total_capital(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, usize> {
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
            .whitebox(controller::contract_obj, |sc| {
                let mut cache = Cache::new(&sc);
                sc.sync_borrow_positions_interest(account_position, &mut cache, true, false);
            });
    }

    pub fn global_sync(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .whitebox(controller::contract_obj, |sc| {
                let mut cache = Cache::new(&sc);
                let account_attributes = sc.account_attributes(account_position).get();
                sc.sync_deposit_positions_interest(
                    account_position,
                    &mut cache,
                    true,
                    &account_attributes,
                );
            });
    }

    pub fn deposit_positions(
        &mut self,
        nonce: u64,
    ) -> MultiValueEncoded<
        StaticApi,
        MultiValue2<EgldOrEsdtTokenIdentifier<StaticApi>, AccountPosition<StaticApi>>,
    > {
        let query = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .deposit_positions(nonce)
            .returns(ReturnsResult)
            .run();

        return query;
    }

    pub fn update_account_threshold(
        &mut self,
        asset_id: EgldOrEsdtTokenIdentifier<StaticApi>,
        has_risks: bool,
        account_nonces: MultiValueEncoded<StaticApi, u64>,
        error_message: Option<&[u8]>,
    ) {
        let call = self
            .world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .update_account_threshold(asset_id, has_risks, account_nonces);

        if error_message.is_some() {
            call.returns(ExpectMessage(
                core::str::from_utf8(error_message.unwrap()).unwrap(),
            ))
            .run();
        } else {
            call.run();
        };
    }

    pub fn update_markets(
        &mut self,
        from: &TestAddress,
        markets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenIdentifier<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .update_indexes(markets)
            .run();
    }

    pub fn update_account_positions(&mut self, from: &TestAddress, account_position: u64) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .update_account_positions(account_position)
            .run();
    }

    pub fn get_vault_supplied_amount(
        &mut self,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .vault_supplied_amount(token_id)
            .returns(ReturnsResult)
            .run()
    }
    // View functions
    pub fn get_collateral_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let collateral_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_collateral_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run();

        collateral_amount
    }

    pub fn get_collateral_amount_for_token_non_existing(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
        error_message: &[u8],
    ) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_collateral_amount_for_token(account_position, token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn get_borrow_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let token_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_borrow_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run();

        token_amount
    }

    pub fn get_borrow_amount_for_token_non_existing(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
        error_message: &[u8],
    ) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_borrow_amount_for_token(account_position, token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    pub fn get_used_isolated_asset_debt_usd(
        &mut self,
        token_id: &TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .isolated_asset_debt_usd(token_id)
            .returns(ReturnsResult)
            .run()
    }

    pub fn get_total_borrow_in_egld_big(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let borrow_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_total_borrow_in_egld(account_position)
            .returns(ReturnsResult)
            .run();

        borrow_amount
    }

    pub fn get_total_borrow_in_egld(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let borrow_amount = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_total_borrow_in_egld(account_position)
            .returns(ReturnsResult)
            .run();

        borrow_amount
    }

    pub fn get_total_collateral_in_egld_big(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let collateral_amount_egld = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_total_collateral_in_egld(account_position)
            .returns(ReturnsResult)
            .run();

        collateral_amount_egld
    }

    pub fn get_total_collateral_in_egld(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let collateral_amount_egld = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_total_collateral_in_egld(account_position)
            .returns(ReturnsResult)
            .run();

        collateral_amount_egld
    }

    pub fn get_liquidation_collateral_available(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        let liquidation_collateral_available = self
            .world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .get_liquidation_collateral_available(account_position)
            .returns(ReturnsResult)
            .run();

        liquidation_collateral_available
    }
}

pub fn setup_lending_pool(
    world: &mut ScenarioWorld,
    template_address_liquidity_pool: &ManagedAddress<StaticApi>,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<controller::ContractObj<DebugApi>>,
    ManagedAddress<StaticApi>,
    ManagedAddress<StaticApi>,
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
        WhiteboxContract::new(LENDING_POOL_ADDRESS, controller::contract_obj);

    let safe_view_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_xexchange_pair::PairProxy)
        .init(
            XEGLD_TOKEN.to_token_identifier(),
            USDC_TOKEN.to_token_identifier(),
            OWNER_ADDRESS,
            OWNER_ADDRESS,
            0u64,
            0u64,
            OWNER_ADDRESS,
            MultiValueEncoded::new(),
        )
        .code(SAFE_PRICE_VIEW_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    let lending_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_lending_pool::ControllerProxy)
        .init(
            template_address_liquidity_pool,
            price_aggregator_sc,
            safe_view_sc.clone(),
            safe_view_sc.clone(), // TODO: Add real accumulator
            safe_view_sc.clone(), // TODO Add wrap SC for WEGLD
            safe_view_sc.clone(), // TODO: Add ash SC
        )
        .code(LENDING_POOL_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world.set_esdt_local_roles(lending_sc.clone(), ACCOUNT_TOKEN.as_bytes(), NFT_ROLES);

    // Set the token id for the account token
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc.clone())
        .whitebox(controller::contract_obj, |sc| {
            sc.account_token()
                .set_token_id(ACCOUNT_TOKEN.to_token_identifier());
        });

    let (xegld_liquid_staking_sc, _) = setup_egld_liquid_staking(world);
    let (lxoxno_liquid_staking_sc, _) = setup_xoxno_liquid_staking(world);

    set_oracle_token_data(
        world,
        &xegld_liquid_staking_sc,
        &lending_sc,
        &lxoxno_liquid_staking_sc,
    );

    let usdc_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(USDC_TOKEN.to_token_identifier()),
        get_usdc_config(),
    );
    let egld_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
        get_egld_config(),
    );
    let xegld_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(XEGLD_TOKEN.to_token_identifier()),
        get_xegld_config(),
    );
    let isolated_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(ISOLATED_TOKEN.to_token_identifier()),
        get_isolated_config(),
    );
    let siloed_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(SILOED_TOKEN.to_token_identifier()),
        get_siloed_config(),
    );
    let capped_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(CAPPED_TOKEN.to_token_identifier()),
        get_capped_config(),
    );
    let segld_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(SEGLD_TOKEN.to_token_identifier()),
        get_segld_config(),
    );
    let legld_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(LEGLD_TOKEN.to_token_identifier()),
        get_legld_config(),
    );

    let xoxno_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(XOXNO_TOKEN.to_token_identifier()),
        get_xoxno_config(),
    );

    let lp_egld_market = setup_market(
        world,
        &lending_sc,
        EgldOrEsdtTokenIdentifier::esdt(LP_EGLD_TOKEN.to_token_identifier()),
        get_legld_config(),
    );

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
        lp_egld_market,
        xoxno_market,
    )
}
pub fn set_oracle_token_data(
    world: &mut ScenarioWorld,
    xegld_liquid_staking_sc: &ManagedAddress<StaticApi>,
    lending_sc: &ManagedAddress<StaticApi>,
    xoxno_liquid_staking_sc: &ManagedAddress<StaticApi>,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            XEGLD_TOKEN.to_token_identifier(),
            18usize,
            xegld_liquid_staking_sc,
            PricingMethod::None,
            OracleType::Derived,
            ExchangeSource::XEGLD,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            LXOXNO_TOKEN.to_token_identifier(),
            18usize,
            xoxno_liquid_staking_sc,
            PricingMethod::None,
            OracleType::Derived,
            ExchangeSource::LXOXNO,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_usdc_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &USDC_TOKEN,
        USDC_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        USDC_PRICE_IN_DOLLARS,
    );
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            LP_EGLD_TOKEN,
            EGLD_DECIMALS as u8,
            &wegld_usdc_pair_sc,
            PricingMethod::None,
            OracleType::Lp,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            EgldOrEsdtTokenIdentifier::egld(),
            EGLD_DECIMALS as u8,
            &wegld_usdc_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            EgldOrEsdtTokenIdentifier::esdt(EGLD_TOKEN.to_token_identifier()),
            EGLD_DECIMALS as u8,
            &wegld_usdc_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            USDC_TOKEN.to_token_identifier(),
            USDC_DECIMALS as u8,
            &wegld_usdc_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_isolated_pair_sc = deploy_pair_sc(
        world,
        &ISOLATED_TOKEN,
        ISOLATED_DECIMALS,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &LP_EGLD_TOKEN,
        ISOLATED_PRICE_IN_DOLLARS,
        EGLD_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            ISOLATED_TOKEN.to_token_identifier(),
            ISOLATED_DECIMALS as u8,
            &wegld_isolated_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_siloed_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &SILOED_TOKEN,
        SILOED_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        SILOED_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            SILOED_TOKEN.to_token_identifier(),
            SILOED_DECIMALS as u8,
            &wegld_siloed_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_capped_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &CAPPED_TOKEN,
        CAPPED_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        CAPPED_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            CAPPED_TOKEN.to_token_identifier(),
            CAPPED_DECIMALS as u8,
            &wegld_capped_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_segld_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &SEGLD_TOKEN,
        SEGLD_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        SEGLD_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            SEGLD_TOKEN.to_token_identifier(),
            SEGLD_DECIMALS as u8,
            &wegld_segld_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_legld_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &LEGLD_TOKEN,
        LEGLD_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        LEGLD_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            LEGLD_TOKEN.to_token_identifier(),
            LEGLD_DECIMALS as u8,
            &wegld_legld_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();

    let wegld_xoxno_pair_sc = deploy_pair_sc(
        world,
        &WEGLD_TOKEN,
        EGLD_DECIMALS,
        &XOXNO_TOKEN,
        XOXNO_DECIMALS,
        &LP_EGLD_TOKEN,
        EGLD_PRICE_IN_DOLLARS,
        XOXNO_PRICE_IN_DOLLARS,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            XOXNO_TOKEN.to_token_identifier(),
            XOXNO_DECIMALS as u8,
            &wegld_xoxno_pair_sc,
            PricingMethod::Aggregator,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            OptionalValue::<usize>::None,
        )
        .run();
}

pub fn deploy_pair_sc(
    world: &mut ScenarioWorld,
    first_token: &TestTokenIdentifier,
    first_token_decimals: usize,
    second_token: &TestTokenIdentifier,
    second_token_decimals: usize,
    lp_token: &TestTokenIdentifier,
    first_token_price_usd: u64,
    second_token_price_usd: u64,
) -> ManagedAddress<StaticApi> {
    let sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_xexchange_pair::PairProxy)
        .init(
            first_token.to_token_identifier(),
            second_token.to_token_identifier(),
            OWNER_ADDRESS,
            OWNER_ADDRESS,
            0u64,
            0u64,
            OWNER_ADDRESS,
            MultiValueEncoded::new(),
        )
        .code(PAIR_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world.set_esdt_local_roles(sc.clone(), lp_token.as_bytes(), ESDT_ROLES);

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(sc.clone())
        .whitebox(pair::contract_obj, |sc| {
            sc.lp_token_identifier().set(lp_token.to_token_identifier());
        });

    let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

    let (first_amount, second_amount) = calculate_optimal_liquidity(
        first_token_price_usd,
        second_token_price_usd,
        first_token_decimals,
        second_token_decimals,
    );

    vec.push(EsdtTokenPayment::new(
        first_token.to_token_identifier(),
        0,
        first_amount,
    ));
    vec.push(EsdtTokenPayment::new(
        second_token.to_token_identifier(),
        0,
        second_amount,
    ));
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(sc.clone())
        .typed(proxy_xexchange_pair::PairProxy)
        .add_initial_liquidity()
        .with_multi_token_transfer(vec)
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(sc.clone())
        .typed(proxy_xexchange_pair::PairProxy)
        .resume()
        .run();
    world.current_block().block_round(1);
    // Do a small swap to initialize first_token price
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(sc.clone())
        .typed(proxy_xexchange_pair::PairProxy)
        .swap_tokens_fixed_input(first_token.to_token_identifier(), BigUint::from(1u64))
        .single_esdt(
            &second_token.to_token_identifier(),
            0u64,
            &BigUint::from(1000000u64),
        )
        .run();

    world.current_block().block_round(10);
    sc.clone()
}

pub fn calculate_optimal_liquidity(
    first_token_price_usd: u64,
    second_token_price_usd: u64,
    first_token_decimals: usize,
    second_token_decimals: usize,
) -> (BigUint<StaticApi>, BigUint<StaticApi>) {
    // We want deep liquidity but not too deep to save on gas
    // Let's use equivalent of $100,000 worth of liquidity
    const TARGET_LIQUIDITY_USD: u64 = 10_000;

    // Calculate how many tokens we need of each to maintain the price ratio
    let first_token_amount = TARGET_LIQUIDITY_USD / first_token_price_usd;
    let second_token_amount = TARGET_LIQUIDITY_USD / second_token_price_usd;

    // Add asset_decimals
    let first_amount = BigUint::from(first_token_amount)
        .mul(BigUint::from(10u64).pow(first_token_decimals as u32));
    let second_amount = BigUint::from(second_token_amount)
        .mul(BigUint::from(10u64).pow(second_token_decimals as u32));

    (first_amount, second_amount)
}

pub fn setup_flash_mock(world: &mut ScenarioWorld) -> ManagedAddress<StaticApi> {
    let flash_mock = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_flash_mock::FlashMockProxy)
        .init()
        .code(FLASH_MOCK_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    flash_mock
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
    );

    submit_price(
        world,
        &price_aggregator_sc,
        SEGLD_TICKER,
        SEGLD_PRICE_IN_DOLLARS,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        LEGLD_TICKER,
        LEGLD_PRICE_IN_DOLLARS,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        USDC_TICKER,
        USDC_PRICE_IN_DOLLARS,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        XEGLD_TICKER,
        XEGLD_PRICE_IN_DOLLARS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        ISOLATED_TICKER,
        ISOLATED_PRICE_IN_DOLLARS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        SILOED_TICKER,
        SILOED_PRICE_IN_DOLLARS,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        CAPPED_TICKER,
        CAPPED_PRICE_IN_DOLLARS,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        XOXNO_TICKER,
        XOXNO_PRICE_IN_DOLLARS,
    );

    (price_aggregator_sc, price_aggregator_whitebox)
}

pub fn setup_egld_liquid_staking(
    world: &mut ScenarioWorld,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<rs_liquid_staking_sc::ContractObj<DebugApi>>,
) {
    let egld_liquid_staking_whitebox = WhiteboxContract::new(
        EGLD_LIQUID_STAKING_ADDRESS,
        rs_liquid_staking_sc::contract_obj,
    );

    let egld_liquid_staking_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_liquid_staking::LiquidStakingProxy)
        .init(
            EGLD_LIQUID_STAKING_ADDRESS,
            BigUint::zero(),
            BigUint::from(25u64),
            100usize,
            0u64,
        )
        .code(EGLD_LIQUID_STAKING_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(egld_liquid_staking_sc.clone())
        .whitebox(rs_liquid_staking_sc::contract_obj, |sc| {
            sc.ls_token()
                .set_token_id(XEGLD_TOKEN.to_token_identifier())
        });

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(egld_liquid_staking_sc.clone())
        .whitebox(rs_liquid_staking_sc::contract_obj, |sc| {
            sc.unstake_token()
                .set_token_id(XEGLD_TOKEN.to_token_identifier())
        });

    world.set_esdt_local_roles(
        egld_liquid_staking_sc.clone(),
        XEGLD_TOKEN.as_bytes(),
        ESDT_ROLES,
    );
    world.set_esdt_local_roles(
        egld_liquid_staking_sc.clone(),
        UNSTAKE_TOKEN.as_bytes(),
        SFT_ROLES,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(egld_liquid_staking_sc.clone())
        .typed(proxy_liquid_staking::LiquidStakingProxy)
        .set_state_active()
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(egld_liquid_staking_sc.clone())
        .typed(proxy_liquid_staking::LiquidStakingProxy)
        .set_scoring_config(ScoringConfig {
            min_nodes: 0u64,
            max_nodes: 100u64,
            min_apy: 0u64,
            max_apy: 100u64,
            stake_weight: 50u64,
            apy_weight: 25u64,
            nodes_weight: 25u64,
            max_score_per_category: 100u64,
            exponential_base: 2u64,
            apy_growth_multiplier: 1u64,
        })
        .run();

    (egld_liquid_staking_sc, egld_liquid_staking_whitebox)
}

pub fn setup_xoxno_liquid_staking(
    world: &mut ScenarioWorld,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<rs_liquid_xoxno::ContractObj<DebugApi>>,
) {
    let xoxno_liquid_staking_whitebox =
        WhiteboxContract::new(XOXNO_LIQUID_STAKING_ADDRESS, rs_liquid_xoxno::contract_obj);

    let xoxno_liquid_staking_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
        .init(XOXNO_TOKEN)
        .code(XOXNO_LIQUID_STAKING_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(xoxno_liquid_staking_sc.clone())
        .whitebox(rs_liquid_xoxno::contract_obj, |sc| {
            sc.main_token().set(XOXNO_TOKEN.to_token_identifier());
        });

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(xoxno_liquid_staking_sc.clone())
        .whitebox(rs_liquid_xoxno::contract_obj, |sc| {
            sc.unstake_token()
                .set_token_id(UXOXNO_TOKEN.to_token_identifier())
        });

    world.set_esdt_local_roles(
        xoxno_liquid_staking_sc.clone(),
        LXOXNO_TOKEN.as_bytes(),
        ESDT_ROLES,
    );
    world.set_esdt_local_roles(
        xoxno_liquid_staking_sc.clone(),
        UXOXNO_TOKEN.as_bytes(),
        SFT_ROLES,
    );

    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(xoxno_liquid_staking_sc.clone())
        .typed(rs_xoxno_proxy::RsLiquidXoxnoProxy)
        .set_state_active()
        .run();

    (xoxno_liquid_staking_sc, xoxno_liquid_staking_whitebox)
}

pub fn submit_price(
    world: &mut ScenarioWorld,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
    from: &[u8],
    price: u64,
) -> () {
    let oracles = vec![
        ORACLE_ADDRESS_1,
        ORACLE_ADDRESS_2,
        ORACLE_ADDRESS_3,
        ORACLE_ADDRESS_4,
    ];

    // world.current_block().block_timestamp(1740184106);
    for oracle in oracles {
        world
            .tx()
            .from(oracle)
            .to(price_aggregator_sc)
            .typed(proxy_aggregator::PriceAggregatorProxy)
            .submit(
                ManagedBuffer::from(from),
                ManagedBuffer::from(DOLLAR_TICKER),
                0u64,
                BigUint::from(price).mul(BigUint::from(WAD)),
            )
            .run();
    }
}

pub fn setup_template_liquidity_pool(
    world: &mut ScenarioWorld,
) -> (
    ManagedAddress<StaticApi>,
    WhiteboxContract<liquidity_layer::ContractObj<DebugApi>>,
) {
    let liquidity_pool_whitebox =
        WhiteboxContract::new(LIQUIDITY_POOL_ADDRESS, liquidity_layer::contract_obj);

    let template_address_liquidity_pool = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_liquidity_pool::LiquidityPoolProxy)
        .init(
            USDC_TICKER,
            BigUint::from(R_MAX),
            BigUint::from(R_BASE),
            BigUint::from(R_SLOPE1),
            BigUint::from(R_SLOPE2),
            BigUint::from(R_SLOPE3),
            BigUint::from(U_MID),
            BigUint::from(U_OPTIMAL),
            BigUint::from(RESERVE_FACTOR),
            USDC_DECIMALS,
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
        .typed(proxy_lending_pool::ControllerProxy)
        .add_e_mode_category(
            BigUint::from(LTV),
            BigUint::from(E_MODE_LIQ_THRESOLD),
            BigUint::from(E_MODE_LIQ_BONUS),
        )
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
        .typed(proxy_lending_pool::ControllerProxy)
        .add_asset_to_e_mode_category(asset, category_id, can_be_collateral, can_be_borrowed)
        .returns(ReturnsResult)
        .run();
}

pub fn setup_market(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    token: EgldOrEsdtTokenIdentifier<StaticApi>,
    config: SetupConfig,
) -> ManagedAddress<StaticApi> {
    let market_address = world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .create_liquidity_pool(
            token,
            BigUint::from(R_MAX),
            BigUint::from(R_BASE),
            BigUint::from(R_SLOPE1),
            BigUint::from(R_SLOPE2),
            BigUint::from(R_SLOPE3),
            BigUint::from(U_MID),
            BigUint::from(U_OPTIMAL),
            BigUint::from(RESERVE_FACTOR),
            config.config.loan_to_value.into_raw_units(),
            config.config.liquidation_threshold.into_raw_units(),
            config.config.liquidation_bonus.into_raw_units(),
            config.config.liquidation_fees.into_raw_units(),
            config.config.is_collateralizable,
            config.config.is_borrowable,
            config.config.is_isolated_asset,
            config.config.isolation_debt_ceiling_usd.into_raw_units(),
            config.config.flashloan_fee.into_raw_units(),
            config.config.is_siloed_borrowing,
            config.config.is_flashloanable,
            config.config.isolation_borrow_enabled,
            config.asset_decimals,
            OptionalValue::from(config.config.borrow_cap),
            OptionalValue::from(config.config.supply_cap),
        )
        .returns(ReturnsResult)
        .run();

    market_address
}

// Helper function for account setup
pub fn setup_accounts(
    state: &mut LendingPoolTestState,
    supplier: TestAddress,
    borrower: TestAddress,
) {
    state
        .world
        .account(supplier)
        .nonce(1)
        .esdt_balance(
            LP_EGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(1000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            XOXNO_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(XOXNO_DECIMALS as u32),
        )
        .esdt_balance(
            ISOLATED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32),
        )
        .esdt_balance(
            CAPPED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(CAPPED_DECIMALS as u32),
        )
        .esdt_balance(
            SILOED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SILOED_DECIMALS as u32),
        )
        .esdt_balance(
            XEGLD_TOKEN,
            BigUint::from(10000000u64) * BigUint::from(10u64).pow(XEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            SEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(1000000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );

    state
        .world
        .account(borrower)
        .nonce(1)
        .esdt_balance(
            LP_EGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(588649983367169591u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            XOXNO_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(XOXNO_DECIMALS as u32),
        )
        .esdt_balance(
            CAPPED_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(CAPPED_DECIMALS as u32),
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
            BigUint::from(10000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            SEGLD_TOKEN,
            BigUint::from(1000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(1000000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );
}

pub fn setup_owner(world: &mut ScenarioWorld) {
    world
        .account(OWNER_ADDRESS)
        .nonce(1)
        .esdt_balance(
            WEGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            EGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
        )
        .esdt_balance(
            XOXNO_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(XOXNO_DECIMALS as u32),
        )
        .esdt_balance(
            ISOLATED_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32),
        )
        .esdt_balance(
            CAPPED_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(CAPPED_DECIMALS as u32),
        )
        .esdt_balance(
            SILOED_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(SILOED_DECIMALS as u32),
        )
        .esdt_balance(
            XEGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(XEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            SEGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            LEGLD_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(LEGLD_DECIMALS as u32),
        )
        .esdt_balance(
            USDC_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
        );
}

pub fn setup_flasher(world: &mut ScenarioWorld, flash: ManagedAddress<StaticApi>) {
    world.set_esdt_balance(
        flash,
        &EGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
}
