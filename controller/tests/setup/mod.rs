use crate::{
    constants::*,
    proxys::{
        proxy_aggregator, proxy_flash_mock, proxy_lending_pool, proxy_liquidity_pool,
        proxy_swap_mock,
    },
};
use common_constants::{EGLD_TICKER, MIN_FIRST_TOLERANCE, MIN_LAST_TOLERANCE};

use multiversx_sc::{
    imports::{MultiValue2, OptionalValue},
    types::{
        BigUint, EgldOrEsdtTokenPayment, ManagedAddress, ManagedArgBuffer, ManagedBuffer,
        ManagedDecimal, MultiValueEncoded, NumDecimals, ReturnsNewManagedAddress, ReturnsResult,
        TestTokenIdentifier, TokenIdentifier,
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

use common_structs::{AccountAttributes, OracleProvider};
use controller::*;
use multiversx_sc::types::{
    EgldOrEsdtTokenIdentifier, EsdtLocalRole, EsdtTokenPayment, ManagedVec, TestEsdtTransfer,
};
use multiversx_sc_scenario::imports::{ExpectMessage, TestAddress};

// ============================================
// CONSTANTS
// ============================================

pub static NFT_ROLES: &[EsdtLocalRole] = &[
    EsdtLocalRole::NftCreate,
    EsdtLocalRole::Mint,
    EsdtLocalRole::NftBurn,
    EsdtLocalRole::NftUpdateAttributes,
];

// ============================================
// TEST STATE STRUCTURE
// ============================================

/// Main test state structure containing all the smart contract instances and addresses
pub struct LendingPoolTestState {
    pub world: ScenarioWorld,
    pub accumulator_sc: ManagedAddress<StaticApi>,
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
    pub swap_mock: ManagedAddress<StaticApi>,
}

impl Default for LendingPoolTestState {
    fn default() -> Self {
        Self::new()
    }
}

impl LendingPoolTestState {
    /// Initialize a new test state with all contracts deployed and configured
    pub fn new() -> Self {
        let mut world = world();
        setup_owner(&mut world);
        world.current_block().block_timestamp(0);

        let template_address_liquidity_pool = setup_template_liquidity_pool(&mut world);

        let price_aggregator_sc = setup_price_aggregator(&mut world);

        let accumulator_sc = setup_accumulator(&mut world);
        let swap_mock = setup_swap_mock(&mut world);
        let (
            lending_sc,
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
            &accumulator_sc,
            &swap_mock,
        );

        let flash_mock = setup_flash_mock(&mut world);
        setup_flasher(&mut world, flash_mock.clone());
        setup_swap_mock_owner(&mut world, swap_mock.clone());
        // For LP safe price simulation
        world.current_block().block_round(1500);

        Self {
            world,
            accumulator_sc,
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
            swap_mock,
        }
    }

    // ============================================
    // CORE LENDING OPERATIONS
    // ============================================

    /// Supply asset to the lending pool
    pub fn supply_asset(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

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
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(vec)
            .run();
    }

    /// Supply asset to the lending pool
    pub fn supply_asset_den(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount_to_transfer: BigUint<StaticApi>,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

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
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(vec)
            .run();
    }

    /// Supply asset with error expectation
    pub fn supply_asset_error(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
        error_message: &[u8],
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

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
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Supply multiple assets in bulk
    pub fn supply_bulk(
        &mut self,
        from: &TestAddress,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        assets: ManagedVec<StaticApi, EsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(assets)
            .run();
    }

    /// Supply bulk assets with error expectation
    pub fn supply_bulk_error(
        &mut self,
        from: &TestAddress,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
        assets: ManagedVec<StaticApi, EsdtTokenPayment<StaticApi>>,
        error_message: &[u8],
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        vec.extend(assets);

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Supply empty asset with error expectation
    pub fn supply_empty_asset_error(
        &mut self,
        from: &TestAddress,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Supply with no payments error
    pub fn empty_supply_asset_error(
        &mut self,
        from: &TestAddress,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .supply(OptionalValue::Some(0u64), e_mode_category)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Supply asset with invalid payment count
    pub fn supply_asset_error_payment_count(
        &mut self,
        from: &TestAddress,
        token_id: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        asset_decimals: usize,
        account_nonce: OptionalValue<u64>,
        e_mode_category: OptionalValue<u8>,
        _is_vault: bool,
        error_message: &[u8],
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();

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
            .supply(
                match account_nonce.into_option() {
                    Some(nonce) => OptionalValue::Some(nonce),
                    None => OptionalValue::Some(0),
                },
                e_mode_category,
            )
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Withdraw asset from the lending pool
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

    /// Withdraw asset denominated in base units (no decimal conversion)
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

    /// Withdraw multiple assets
    pub fn withdraw_assets(
        &mut self,
        from: &TestAddress,
        assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
        account_nonce: u64,
    ) {
        let transfer = EsdtTokenPayment::new(
            ACCOUNT_TOKEN.to_token_identifier(),
            account_nonce,
            BigUint::from(1u64),
        );

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .withdraw(assets)
            .esdt(transfer)
            .run();
    }

    /// Withdraw asset with error expectation
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

    /// Borrow asset from the lending pool
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

    pub fn borrow_asset_den(
        &mut self,
        from: &TestAddress,
        asset_to_borrow: TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let asset = EgldOrEsdtTokenPayment::new(
            EgldOrEsdtTokenIdentifier::esdt(asset_to_borrow.to_token_identifier()),
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
            .borrow(array)
            .esdt(TestEsdtTransfer(ACCOUNT_TOKEN, account_nonce, 1u64))
            .run();
    }

    /// Borrow multiple assets
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

    /// Borrow asset with error expectation
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

    /// Borrow multiple assets with error expectation
    pub fn borrow_assets_error(
        &mut self,
        account_nonce: u64,
        from: &TestAddress,
        assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow(assets)
            .esdt(TestEsdtTransfer(ACCOUNT_TOKEN, account_nonce, 1u64))
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Repay borrowed asset
    pub fn repay_asset(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .repay(account_nonce)
            .esdt(EsdtTokenPayment::new(
                token.to_token_identifier(),
                0,
                amount_to_transfer,
            ))
            .run();
    }

    /// Repay asset denominated in base units
    pub fn repay_asset_deno(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .repay(account_nonce)
            .esdt(EsdtTokenPayment::new(
                token.to_token_identifier(),
                0,
                amount,
            ))
            .run();
    }

    /// Repay asset with error expectation
    pub fn repay_asset_error(
        &mut self,
        from: &TestAddress,
        token: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
        error_message: &[u8],
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .repay(account_nonce)
            .esdt(EsdtTokenPayment::new(
                token.to_token_identifier(),
                0,
                amount_to_transfer,
            ))
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Liquidate an account position
    pub fn liquidate_account(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        vec.push(EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .multi_esdt(vec)
            .run();
    }

    /// Liquidate account denominated in base units
    pub fn liquidate_account_den(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount_to_transfer: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        vec.push(EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .multi_esdt(vec)
            .run();
    }

    /// Liquidate account with multiple payments
    pub fn liquidate_account_dem(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        vec.push(EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .multi_esdt(vec)
            .run();
    }

    /// Liquidate account with bulk payments
    pub fn liquidate_account_dem_bulk(
        &mut self,
        from: &TestAddress,
        payments: Vec<(&TestTokenIdentifier, &BigUint<StaticApi>)>,
        account_nonce: u64,
    ) {
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        for (token, amount) in payments {
            vec.push(EsdtTokenPayment::new(
                token.to_token_identifier(),
                0,
                amount.clone(),
            ));
        }

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .multi_esdt(vec)
            .run();
    }

    /// Liquidate account with error expectation
    pub fn liquidate_account_error(
        &mut self,
        from: &TestAddress,
        liquidator_payment: &TestTokenIdentifier,
        amount: BigUint<StaticApi>,
        account_nonce: u64,
        asset_decimals: usize,
        error_message: &[u8],
    ) {
        let amount_to_transfer = amount.mul(BigUint::from(10u64).pow(asset_decimals as u32));
        let mut vec = ManagedVec::<StaticApi, EsdtTokenPayment<StaticApi>>::new();
        vec.push(EsdtTokenPayment::new(
            liquidator_payment.to_token_identifier(),
            0,
            amount_to_transfer,
        ));

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidate(account_nonce)
            .multi_esdt(vec)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Execute flash loan
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
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .flash_loan(
                token.to_token_identifier(),
                amount,
                contract,
                endpoint,
                arguments,
            )
            .run();
    }

    /// Execute flash loan with error expectation
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
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .flash_loan(
                token.to_token_identifier(),
                amount,
                contract,
                endpoint,
                arguments,
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Clean bad debt from an account
    pub fn clean_bad_debt(&mut self, account_position: u64) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .clean_bad_debt(account_position)
            .run();
    }

    /// Update account threshold
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

        if let Some(err_msg) = error_message {
            call.returns(ExpectMessage(core::str::from_utf8(err_msg).unwrap()))
                .run();
        } else {
            call.run();
        }
    }

    /// Update market indexes
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

    // ============================================
    // CONFIGURATION ENDPOINTS
    // ============================================

    /// Register account token
    pub fn register_account_token(
        &mut self,
        token_name: ManagedBuffer<StaticApi>,
        ticker: ManagedBuffer<StaticApi>,
        payment_amount: BigUint<StaticApi>,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .register_account_token(token_name, ticker)
            .egld(payment_amount)
            .run();
    }

    /// Set token oracle configuration
    pub fn set_token_oracle(
        &mut self,
        market_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        decimals: usize,
        contract_address: &ManagedAddress<StaticApi>,
        pricing_method: PricingMethod,
        oracle_type: OracleType,
        source: ExchangeSource,
        first_tolerance: BigUint<StaticApi>,
        last_tolerance: BigUint<StaticApi>,
        max_price_stale_seconds: u64,
        one_dex_pair_id: OptionalValue<usize>,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_token_oracle(
                market_token.clone(),
                decimals,
                contract_address.clone(),
                pricing_method,
                oracle_type,
                source,
                first_tolerance,
                last_tolerance,
                max_price_stale_seconds,
                one_dex_pair_id,
            )
            .run();
    }

    /// Set token oracle configuration with error
    pub fn set_token_oracle_error(
        &mut self,
        market_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        decimals: usize,
        contract_address: &ManagedAddress<StaticApi>,
        pricing_method: PricingMethod,
        oracle_type: OracleType,
        source: ExchangeSource,
        first_tolerance: BigUint<StaticApi>,
        last_tolerance: BigUint<StaticApi>,
        max_price_stale_seconds: u64,
        one_dex_pair_id: OptionalValue<usize>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_token_oracle(
                market_token.clone(),
                decimals,
                contract_address.clone(),
                pricing_method,
                oracle_type,
                source,
                first_tolerance,
                last_tolerance,
                max_price_stale_seconds,
                one_dex_pair_id,
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Edit token oracle tolerance
    pub fn edit_token_oracle_tolerance(
        &mut self,
        market_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        first_tolerance: BigUint<StaticApi>,
        last_tolerance: BigUint<StaticApi>,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_token_oracle_tolerance(market_token.clone(), first_tolerance, last_tolerance)
            .run();
    }

    /// Edit token oracle tolerance with error
    pub fn edit_token_oracle_tolerance_error(
        &mut self,
        market_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        first_tolerance: BigUint<StaticApi>,
        last_tolerance: BigUint<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_token_oracle_tolerance(market_token.clone(), first_tolerance, last_tolerance)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Set price aggregator address
    pub fn set_aggregator(&mut self, aggregator: ManagedAddress<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_aggregator(aggregator)
            .run();
    }

    /// Set price aggregator address with error
    pub fn set_aggregator_error(
        &mut self,
        aggregator: ManagedAddress<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_aggregator(aggregator)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Set swap router address
    pub fn set_swap_router(&mut self, address: ManagedAddress<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_swap_router(address)
            .run();
    }

    /// Set swap router address with error
    pub fn set_swap_router_error(
        &mut self,
        address: ManagedAddress<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_swap_router(address)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Set accumulator address
    pub fn set_accumulator(&mut self, accumulator: ManagedAddress<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_accumulator(accumulator)
            .run();
    }

    /// Set accumulator address with error
    pub fn set_accumulator_error(
        &mut self,
        accumulator: ManagedAddress<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_accumulator(accumulator)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Set safe price view address
    pub fn set_safe_price_view(&mut self, safe_view_address: ManagedAddress<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_safe_price_view(safe_view_address)
            .run();
    }

    /// Set safe price view address with error
    pub fn set_safe_price_view_error(
        &mut self,
        safe_view_address: ManagedAddress<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_safe_price_view(safe_view_address)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Set position limits
    pub fn set_position_limits(&mut self, max_borrow_positions: u8, max_supply_positions: u8) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_position_limits(max_borrow_positions, max_supply_positions)
            .run();
    }

    /// Set liquidity pool template address
    pub fn set_liquidity_pool_template(&mut self, address: ManagedAddress<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_liquidity_pool_template(address)
            .run();
    }

    /// Set liquidity pool template address with error
    pub fn set_liquidity_pool_template_error(
        &mut self,
        address: ManagedAddress<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .set_liquidity_pool_template(address)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    // ============================================
    // MARKET MANAGEMENT ENDPOINTS
    // ============================================

    /// Claim revenue from a market
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

    /// Add a new market (create liquidity pool)
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
        let pool_address = self
            .world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .create_liquidity_pool(
                token_id.clone(),
                BigUint::from(max_borrow_rate),
                BigUint::from(base_borrow_rate),
                BigUint::from(slope1),
                BigUint::from(slope2),
                BigUint::from(slope3),
                BigUint::from(mid_utilization),
                BigUint::from(optimal_utilization),
                BigUint::from(reserve_factor),
                config.loan_to_value_bps.into_raw_units(),
                config.liquidation_threshold_bps.into_raw_units(),
                config.liquidation_bonus_bps.into_raw_units(),
                config.liquidation_fees_bps.into_raw_units(),
                config.is_collateralizable,
                config.is_borrowable,
                config.is_isolated_asset,
                config.isolation_debt_ceiling_usd_wad.into_raw_units(),
                config.flashloan_fee_bps.into_raw_units(),
                config.is_siloed_borrowing,
                config.is_flashloanable,
                config.isolation_borrow_enabled,
                asset_decimals,
                config.borrow_cap_wad.unwrap_or(BigUint::zero()),
                config.supply_cap_wad.unwrap_or(BigUint::zero()),
            )
            .returns(ReturnsNewManagedAddress)
            .run();

        if token_id.is_egld() {
            world().set_esdt_balance(
                pool_address.clone(),
                EGLD_TICKER,
                &BigUint::from(10u64).pow(asset_decimals as u32) * 100000000u64,
            );
        } else {
            let token_bytes = token_id
                .as_esdt_option()
                .unwrap()
                .as_managed_buffer()
                .to_boxed_bytes();
            world().set_esdt_balance(
                pool_address.clone(),
                token_bytes.as_slice(),
                &BigUint::from(10u64).pow(asset_decimals as u32) * 100000000u64,
            );
        }

        pool_address
    }

    /// Create liquidity pool
    pub fn create_liquidity_pool(
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
    ) -> ManagedAddress<StaticApi> {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .create_liquidity_pool(
                token_id,
                BigUint::from(max_borrow_rate),
                BigUint::from(base_borrow_rate),
                BigUint::from(slope1),
                BigUint::from(slope2),
                BigUint::from(slope3),
                BigUint::from(mid_utilization),
                BigUint::from(optimal_utilization),
                BigUint::from(reserve_factor),
                config.loan_to_value_bps.into_raw_units(),
                config.liquidation_threshold_bps.into_raw_units(),
                config.liquidation_bonus_bps.into_raw_units(),
                config.liquidation_fees_bps.into_raw_units(),
                config.is_collateralizable,
                config.is_borrowable,
                config.is_isolated_asset,
                config.isolation_debt_ceiling_usd_wad.into_raw_units(),
                config.flashloan_fee_bps.into_raw_units(),
                config.is_siloed_borrowing,
                config.is_flashloanable,
                config.isolation_borrow_enabled,
                18usize, // Default decimals, should be passed as parameter
                config.borrow_cap_wad.unwrap_or(BigUint::zero()),
                config.supply_cap_wad.unwrap_or(BigUint::zero()),
            )
            .returns(ReturnsNewManagedAddress)
            .run()
    }

    /// Create liquidity pool with error
    pub fn create_liquidity_pool_error(
        &mut self,
        token_id: EgldOrEsdtTokenIdentifier<StaticApi>,
        max_borrow_rate: BigUint<StaticApi>,
        base_borrow_rate: BigUint<StaticApi>,
        slope1: BigUint<StaticApi>,
        slope2: BigUint<StaticApi>,
        slope3: BigUint<StaticApi>,
        mid_utilization: BigUint<StaticApi>,
        optimal_utilization: BigUint<StaticApi>,
        reserve_factor: BigUint<StaticApi>,
        ltv: BigUint<StaticApi>,
        liquidation_threshold: BigUint<StaticApi>,
        liquidation_bonus: BigUint<StaticApi>,
        liquidation_fees: BigUint<StaticApi>,
        is_collateralizable: bool,
        is_borrowable: bool,
        is_isolated_asset: bool,
        isolation_debt_ceiling_usd: BigUint<StaticApi>,
        flashloan_fee: BigUint<StaticApi>,
        is_siloed_borrowing: bool,
        is_flashloanable: bool,
        isolation_borrow_enabled: bool,
        asset_decimals: usize,
        borrow_cap: BigUint<StaticApi>,
        supply_cap: BigUint<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
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
                ltv,
                liquidation_threshold,
                liquidation_bonus,
                liquidation_fees,
                is_collateralizable,
                is_borrowable,
                is_isolated_asset,
                isolation_debt_ceiling_usd,
                flashloan_fee,
                is_siloed_borrowing,
                is_flashloanable,
                isolation_borrow_enabled,
                asset_decimals,
                borrow_cap,
                supply_cap,
            )
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Upgrade liquidity pool configuration
    pub fn upgrade_liquidity_pool_params(
        &mut self,
        base_asset: &EgldOrEsdtTokenIdentifier<StaticApi>,
        max_borrow_rate: BigUint<StaticApi>,
        base_borrow_rate: BigUint<StaticApi>,
        slope1: BigUint<StaticApi>,
        slope2: BigUint<StaticApi>,
        slope3: BigUint<StaticApi>,
        mid_utilization: BigUint<StaticApi>,
        optimal_utilization: BigUint<StaticApi>,
        reserve_factor: BigUint<StaticApi>,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .upgrade_liquidity_pool_params(
                base_asset.clone(),
                max_borrow_rate,
                base_borrow_rate,
                slope1,
                slope2,
                slope3,
                mid_utilization,
                optimal_utilization,
                reserve_factor,
            )
            .run();
    }

    /// Edit asset configuration
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
            .from(OWNER_ADDRESS)
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_asset_config(
                asset,
                loan_to_value.clone(),
                liquidation_threshold.clone(),
                liquidation_bonus.clone(),
                liquidation_fees.clone(),
                is_isolated_asset,
                isolation_debt_ceiling_usd.clone(),
                is_siloed_borrowing,
                is_flashloanable,
                flashloan_fee.clone(),
                is_collateralizable,
                is_borrowable,
                isolation_borrow_enabled,
                borrow_cap.clone(),
                supply_cap.clone(),
            );

        if let Some(err_msg) = error_message {
            call.returns(ExpectMessage(core::str::from_utf8(err_msg).unwrap()))
                .run();
        } else {
            call.run();
        }
    }

    // ============================================
    // E-MODE CONFIGURATION
    // ============================================

    /// Add e-mode category
    pub fn add_e_mode_category(
        &mut self,
        ltv: BigUint<StaticApi>,
        liquidation_threshold: BigUint<StaticApi>,
        liquidation_bonus: BigUint<StaticApi>,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .add_e_mode_category(ltv, liquidation_threshold, liquidation_bonus)
            .run();
    }

    /// Add e-mode category with error
    pub fn add_e_mode_category_error(
        &mut self,
        ltv: BigUint<StaticApi>,
        liquidation_threshold: BigUint<StaticApi>,
        liquidation_bonus: BigUint<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .add_e_mode_category(ltv, liquidation_threshold, liquidation_bonus)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Edit e-mode category
    pub fn edit_e_mode_category(&mut self, category: EModeCategory<StaticApi>) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_e_mode_category(category)
            .run();
    }

    /// Edit e-mode category with error
    pub fn edit_e_mode_category_error(
        &mut self,
        category: EModeCategory<StaticApi>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_e_mode_category(category)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Remove e-mode category
    pub fn remove_e_mode_category(&mut self, category_id: u8) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .remove_e_mode_category(category_id)
            .run();
    }

    /// Remove e-mode category with error
    pub fn remove_e_mode_category_error(&mut self, category_id: u8, error_message: &[u8]) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .remove_e_mode_category(category_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Add asset to e-mode category
    pub fn add_asset_to_e_mode_category(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
        can_be_collateral: bool,
        can_be_borrowed: bool,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .add_asset_to_e_mode_category(asset, category_id, can_be_collateral, can_be_borrowed)
            .run();
    }

    /// Add asset to e-mode category with error
    pub fn add_asset_to_e_mode_category_error(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
        can_be_collateral: bool,
        can_be_borrowed: bool,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .add_asset_to_e_mode_category(asset, category_id, can_be_collateral, can_be_borrowed)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Edit asset in e-mode category
    pub fn edit_asset_in_e_mode_category(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
        config: EModeAssetConfig,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_asset_in_e_mode_category(asset, category_id, config)
            .run();
    }

    /// Edit asset in e-mode category with error
    pub fn edit_asset_in_e_mode_category_error(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
        config: EModeAssetConfig,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .edit_asset_in_e_mode_category(asset, category_id, config)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Remove asset from e-mode category
    pub fn remove_asset_from_e_mode_category(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .remove_asset_from_e_mode_category(asset, category_id)
            .run();
    }

    /// Remove asset from e-mode category with error
    pub fn remove_asset_from_e_mode_category_error(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
        category_id: u8,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(OWNER_ADDRESS)
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .remove_asset_from_e_mode_category(asset, category_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    // ============================================
    // STRATEGY ENDPOINTS
    // ============================================

    /// Multiply position (leverage)
    pub fn multiply(
        &mut self,
        from: &TestAddress,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        debt_to_flash_loan: BigUint<StaticApi>,
        debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        mode: PositionMode,
        steps: ManagedArgBuffer<StaticApi>,
        steps_payment: OptionalValue<ManagedArgBuffer<StaticApi>>,
        payments: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .multiply(
                e_mode_category,
                collateral_token,
                debt_to_flash_loan,
                debt_token,
                mode,
                steps,
                steps_payment,
            )
            .payment(payments)
            .run();
    }

    /// Multiply with error expectation
    pub fn multiply_error(
        &mut self,
        from: &TestAddress,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        debt_to_flash_loan: BigUint<StaticApi>,
        debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        mode: PositionMode,
        steps: ManagedArgBuffer<StaticApi>,
        steps_payment: OptionalValue<ManagedArgBuffer<StaticApi>>,
        payments: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .multiply(
                e_mode_category,
                collateral_token,
                debt_to_flash_loan,
                debt_token,
                mode,
                steps,
                steps_payment,
            )
            .payment(payments)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Swap debt
    pub fn swap_debt(
        &mut self,
        from: &TestAddress,
        existing_debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        new_debt_amount_raw: &BigUint<StaticApi>,
        new_debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        steps: ManagedArgBuffer<StaticApi>,
        account_payment: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .swap_debt(
                existing_debt_token,
                new_debt_amount_raw,
                new_debt_token,
                steps,
            )
            .payment(account_payment)
            .run();
    }

    /// Swap debt with error expectation
    pub fn swap_debt_error(
        &mut self,
        from: &TestAddress,
        existing_debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        new_debt_amount_raw: &BigUint<StaticApi>,
        new_debt_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        steps: ManagedArgBuffer<StaticApi>,
        account_payment: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .swap_debt(
                existing_debt_token,
                new_debt_amount_raw,
                new_debt_token,
                steps,
            )
            .payment(account_payment)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Swap collateral
    pub fn swap_collateral(
        &mut self,
        from: &TestAddress,
        current_collateral: &EgldOrEsdtTokenIdentifier<StaticApi>,
        from_amount: BigUint<StaticApi>,
        new_collateral: &EgldOrEsdtTokenIdentifier<StaticApi>,
        steps: ManagedArgBuffer<StaticApi>,
        account_payment: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .swap_collateral(current_collateral, from_amount, new_collateral, steps)
            .payment(account_payment)
            .run();
    }

    /// Swap collateral with error expectation
    pub fn swap_collateral_error(
        &mut self,
        from: &TestAddress,
        current_collateral: &EgldOrEsdtTokenIdentifier<StaticApi>,
        from_amount: BigUint<StaticApi>,
        new_collateral: &EgldOrEsdtTokenIdentifier<StaticApi>,
        steps: ManagedArgBuffer<StaticApi>,
        account_payment: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
        error_message: &[u8],
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .swap_collateral(current_collateral, from_amount, new_collateral, steps)
            .payment(account_payment)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Repay debt with collateral
    pub fn repay_debt_with_collateral(
        &mut self,
        from: &TestAddress,
        from_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        from_amount: BigUint<StaticApi>,
        to_token: &EgldOrEsdtTokenIdentifier<StaticApi>,
        close_position: bool,
        steps: OptionalValue<ManagedArgBuffer<StaticApi>>,
        account_payment: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) {
        self.world
            .tx()
            .from(from.to_managed_address())
            .to(&self.lending_sc)
            .typed(proxy_lending_pool::ControllerProxy)
            .repay_debt_with_collateral(from_token, from_amount, to_token, close_position, steps)
            .payment(account_payment)
            .run();
    }

    // ============================================
    // VIEW FUNCTIONS - PRICES AND HEALTH
    // ============================================

    /// Get USD price
    pub fn usd_price(
        &mut self,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .usd_price(token_id)
            .returns(ReturnsResult)
            .run()
    }

    /// Get EGLD price
    pub fn egld_price(
        &mut self,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .egld_price(token_id)
            .returns(ReturnsResult)
            .run()
    }

    /// Get USD price with error
    pub fn usd_price_error(&mut self, token_id: TestTokenIdentifier, error_message: &[u8]) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .usd_price(token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    // /// Get safe price by timestamp offset
    // pub fn safe_price_by_timestamp_offset(
    //     &mut self,
    //     token_id: EgldOrEsdtTokenIdentifier<StaticApi>,
    //     offset: u64,
    // ) -> BigUint<StaticApi> {
    //     self.world
    //         .query()
    //         .to(self.lending_sc.clone())
    //         .typed(proxy_lending_pool::ControllerProxy)
    //         .get_safe_price_by_timestamp_offset(token_id, offset)
    //         .returns(ReturnsResult)
    //         .run()
    // }

    /// Can be liquidated check
    pub fn can_be_liquidated(&mut self, account_position: u64) -> bool {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .can_be_liquidated(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get account health factor
    pub fn account_health_factor(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .health_factor(account_position)
            .returns(ReturnsResult)
            .run()
    }

    // ============================================
    // VIEW FUNCTIONS - ACCOUNT POSITIONS
    // ============================================

    /// Get collateral amount for token
    pub fn collateral_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .collateral_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run()
    }

    /// Get collateral amount for non-existing token
    pub fn collateral_amount_for_token_non_existing(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
        error_message: &[u8],
    ) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .collateral_amount_for_token(account_position, token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Get borrow amount for token
    pub fn borrow_amount_for_token(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow_amount_for_token(account_position, token_id)
            .returns(ReturnsResult)
            .run()
    }

    /// Get borrow amount for non-existing token
    pub fn borrow_amount_for_token_non_existing(
        &mut self,
        account_position: u64,
        token_id: TestTokenIdentifier,
        error_message: &[u8],
    ) {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .borrow_amount_for_token(account_position, token_id)
            .returns(ExpectMessage(core::str::from_utf8(error_message).unwrap()))
            .run();
    }

    /// Get total borrow in EGLD
    pub fn total_borrow_in_egld(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .total_borrow_in_egld(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get total borrow in EGLD (big)
    pub fn total_borrow_in_egld_big(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .total_borrow_in_egld(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get total collateral in EGLD
    pub fn total_collateral_in_egld(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .total_collateral_in_egld(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get total collateral in EGLD (big)
    pub fn total_collateral_in_egld_big(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .total_collateral_in_egld(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get LTV collateral in EGLD
    pub fn ltv_collateral_in_egld(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .ltv_collateral_in_egld(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get liquidation collateral available
    pub fn liquidation_collateral_available(
        &mut self,
        account_position: u64,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidation_collateral_available(account_position)
            .returns(ReturnsResult)
            .run()
    }

    /// Get deposit positions
    pub fn deposit_positions(
        &mut self,
        nonce: u64,
    ) -> MultiValueEncoded<
        StaticApi,
        MultiValue2<EgldOrEsdtTokenIdentifier<StaticApi>, AccountPosition<StaticApi>>,
    > {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .positions(nonce, AccountPositionType::Deposit)
            .returns(ReturnsResult)
            .run()
    }

    /// Get borrow positions
    pub fn borrow_positions(
        &mut self,
        nonce: u64,
    ) -> MultiValueEncoded<
        StaticApi,
        MultiValue2<EgldOrEsdtTokenIdentifier<StaticApi>, AccountPosition<StaticApi>>,
    > {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .positions(nonce, AccountPositionType::Borrow)
            .returns(ReturnsResult)
            .run()
    }

    /// Get liquidation estimations
    pub fn liquidation_estimations(
        &mut self,
        account_nonce: u64,
        debt_payments: ManagedVec<StaticApi, EgldOrEsdtTokenPayment<StaticApi>>,
    ) -> LiquidationEstimate<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liquidation_estimations(account_nonce, debt_payments)
            .returns(ReturnsResult)
            .run()
    }

    // ============================================
    // VIEW FUNCTIONS - MARKET DATA
    // ============================================

    /// Get all market indexes
    pub fn all_market_indexes(
        &mut self,
        assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenIdentifier<StaticApi>>,
    ) -> ManagedVec<StaticApi, MarketIndexView<StaticApi>> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .all_market_indexes(assets)
            .returns(ReturnsResult)
            .run()
    }

    /// Get all markets
    pub fn all_markets(
        &mut self,
        assets: MultiValueEncoded<StaticApi, EgldOrEsdtTokenIdentifier<StaticApi>>,
    ) -> ManagedVec<StaticApi, AssetExtendedConfigView<StaticApi>> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .all_markets(assets)
            .returns(ReturnsResult)
            .run()
    }

    /// Get used isolated asset debt in USD
    pub fn used_isolated_asset_debt_usd(
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

    // ============================================
    // VIEW FUNCTIONS - STORAGE GETTERS
    // ============================================

    /// Get pools list
    pub fn pools(&mut self) -> MultiValueEncoded<StaticApi, ManagedAddress<StaticApi>> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .pools()
            .returns(ReturnsResult)
            .run()
    }

    /// Get account data
    pub fn account(&mut self, account_nonce: u64) -> AccountAttributes<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .account_attributes(account_nonce)
            .returns(ReturnsResult)
            .run()
    }

    /// Get account nonce for address
    pub fn last_account_nonce(&mut self) -> u64 {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .account_nonce()
            .returns(ReturnsResult)
            .run()
    }

    /// Get all accounts
    pub fn accounts(&mut self) -> MultiValueEncoded<StaticApi, u64> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .accounts()
            .returns(ReturnsResult)
            .run()
    }

    /// Get account attributes
    pub fn account_attributes(&mut self, account_nonce: u64) -> AccountAttributes<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .account_attributes(account_nonce)
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_borrow_index(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .borrow_index()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_supply_index(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .supply_index()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_supplied(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .supplied()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_protocol_revenue(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .protocol_revenue()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_borrowed_amount(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .borrowed_amount()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_supplied_amount(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .supplied_amount()
            .returns(ReturnsResult)
            .run()
    }

    pub fn market_borrowed(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .borrowed()
            .returns(ReturnsResult)
            .run()
    }

    /// Add rewards to a market via the controller's `addRewards` endpoint.
    /// Owner-only; attaches a single ESDT payment matching the market asset.
    pub fn add_rewards(
        &mut self,
        from: &TestAddress,
        token: TestTokenIdentifier,
        raw_amount: BigUint<StaticApi>,
    ) {
        use multiversx_sc::types::EsdtTokenPayment;

        self.world
            .tx()
            .from(from.to_managed_address())
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .add_reward()
            .payment(EsdtTokenPayment::new(
                token.to_token_identifier(),
                0,
                raw_amount,
            ))
            .run();
    }

    /// Get positions
    pub fn positions(
        &mut self,
        account_nonce: u64,
        position_type: AccountPositionType,
    ) -> MultiValueEncoded<
        StaticApi,
        MultiValue2<EgldOrEsdtTokenIdentifier<StaticApi>, AccountPosition<StaticApi>>,
    > {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .positions(account_nonce, position_type)
            .returns(ReturnsResult)
            .run()
    }

    /// Get liquidity pool template address
    pub fn liq_pool_template_address(&mut self) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .liq_pool_template_address()
            .returns(ReturnsResult)
            .run()
    }

    /// Get accumulator address
    pub fn accumulator_address(&mut self) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .accumulator_address()
            .returns(ReturnsResult)
            .run()
    }

    /// Get pool address for asset
    pub fn pool_address(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
    ) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .pools_map(asset)
            .returns(ReturnsResult)
            .run()
    }

    /// Get price aggregator address
    pub fn price_aggregator_address(&mut self) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .price_aggregator_address()
            .returns(ReturnsResult)
            .run()
    }

    /// Get safe price address
    pub fn safe_price_address(&mut self) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .safe_price_view()
            .returns(ReturnsResult)
            .run()
    }

    /// Get swap router address
    pub fn swap_router_address(&mut self) -> ManagedAddress<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .swap_router()
            .returns(ReturnsResult)
            .run()
    }

    /// Get asset configuration
    pub fn asset_config(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
    ) -> AssetConfig<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .asset_config(asset)
            .returns(ReturnsResult)
            .run()
    }

    /// Get last e-mode category ID
    pub fn last_e_mode_category_id(&mut self) -> u8 {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .last_e_mode_category_id()
            .returns(ReturnsResult)
            .run()
    }

    /// Get e-modes
    pub fn e_modes(
        &mut self,
    ) -> MultiValueEncoded<StaticApi, MultiValue2<u8, EModeCategory<StaticApi>>> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .e_mode_categories()
            .returns(ReturnsResult)
            .run()
    }

    /// Get asset e-modes
    pub fn asset_e_modes(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
    ) -> MultiValueEncoded<StaticApi, u8> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .asset_e_modes(asset)
            .returns(ReturnsResult)
            .run()
    }

    /// Get e-modes assets
    pub fn e_modes_assets(
        &mut self,
        category_id: u8,
    ) -> MultiValueEncoded<
        StaticApi,
        MultiValue2<EgldOrEsdtTokenIdentifier<StaticApi>, EModeAssetConfig>,
    > {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .e_mode_assets(category_id)
            .returns(ReturnsResult)
            .run()
    }

    /// Get isolated asset debt in USD
    pub fn isolated_asset_debt_usd(
        &mut self,
        asset: EgldOrEsdtTokenIdentifier<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .isolated_asset_debt_usd(asset)
            .returns(ReturnsResult)
            .run()
    }

    /// Get token oracle
    pub fn token_oracle(
        &mut self,
        token: EgldOrEsdtTokenIdentifier<StaticApi>,
    ) -> OracleProvider<StaticApi> {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .token_oracle(token)
            .returns(ReturnsResult)
            .run()
    }

    /// Check if flash loan is ongoing
    pub fn is_flash_loan_ongoing(&mut self) -> bool {
        self.world
            .query()
            .to(self.lending_sc.clone())
            .typed(proxy_lending_pool::ControllerProxy)
            .flash_loan_ongoing()
            .returns(ReturnsResult)
            .run()
    }
}

// ============================================
// SETUP FUNCTIONS
// ============================================

/// Initialize the world with registered contracts
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
    blockchain.register_contract(ACCUMULATOR_PATH, accumulator::ContractBuilder);

    blockchain.register_contract(FLASH_MOCK_PATH, flash_mock::ContractBuilder);
    blockchain.register_contract(SWAP_MOCK_PATH, swap_mock::ContractBuilder);

    blockchain
}

/// Setup the lending pool
pub fn setup_lending_pool(
    world: &mut ScenarioWorld,
    template_address_liquidity_pool: &ManagedAddress<StaticApi>,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
    accumulator_sc: &ManagedAddress<StaticApi>,
    swap_mock_sc: &ManagedAddress<StaticApi>,
) -> (
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
    ManagedAddress<StaticApi>,
) {
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
            accumulator_sc.clone(),
            swap_mock_sc.clone(),
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
            sc.account()
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

/// Set oracle token data
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
            OptionalValue::<usize>::None,
        )
        .run();
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .set_token_oracle(
            EgldOrEsdtTokenIdentifier::esdt(WEGLD_TOKEN.to_token_identifier()),
            EGLD_DECIMALS as u8,
            &wegld_usdc_pair_sc,
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            SECONDS_PER_HOUR * 1000,
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
            PricingMethod::Mix,
            OracleType::Normal,
            ExchangeSource::XExchange,
            BigUint::from(MIN_FIRST_TOLERANCE),
            BigUint::from(MIN_LAST_TOLERANCE),
            SECONDS_PER_HOUR * 1000,
            OptionalValue::<usize>::None,
        )
        .run();
}

/// Deploy pair smart contract
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

/// Calculate optimal liquidity for a pair
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

/// Setup flash mock contract
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

pub fn setup_swap_mock(world: &mut ScenarioWorld) -> ManagedAddress<StaticApi> {
    let swap_mock = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_swap_mock::SwapMockProxy)
        .init()
        .code(SWAP_MOCK_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    swap_mock
}

/// Setup price aggregator
pub fn setup_price_aggregator(world: &mut ScenarioWorld) -> ManagedAddress<StaticApi> {
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
        0u64,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        SEGLD_TICKER,
        SEGLD_PRICE_IN_DOLLARS,
        0u64,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        LEGLD_TICKER,
        LEGLD_PRICE_IN_DOLLARS,
        0u64,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        USDC_TICKER,
        USDC_PRICE_IN_DOLLARS,
        0u64,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        XEGLD_TICKER,
        XEGLD_PRICE_IN_DOLLARS,
        0u64,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        ISOLATED_TICKER,
        ISOLATED_PRICE_IN_DOLLARS,
        0u64,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        SILOED_TICKER,
        SILOED_PRICE_IN_DOLLARS,
        0u64,
    );
    submit_price(
        world,
        &price_aggregator_sc,
        CAPPED_TICKER,
        CAPPED_PRICE_IN_DOLLARS,
        0u64,
    );

    submit_price(
        world,
        &price_aggregator_sc,
        XOXNO_TICKER,
        XOXNO_PRICE_IN_DOLLARS,
        0u64,
    );

    price_aggregator_sc
}

/// Setup EGLD liquid staking
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

/// Setup XOXNO liquid staking
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

    // Mirror xEGLD pattern: set the LS token id (LXOXNO) issued by the liquid staking contract
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(xoxno_liquid_staking_sc.clone())
        .whitebox(rs_liquid_xoxno::contract_obj, |sc| {
            sc.ls_token()
                .set_token_id(LXOXNO_TOKEN.to_token_identifier())
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

/// Submit price to aggregator
pub fn submit_price(
    world: &mut ScenarioWorld,
    price_aggregator_sc: &ManagedAddress<StaticApi>,
    from: &[u8],
    price: u64,
    timestamp: u64,
) {
    let oracles = vec![
        ORACLE_ADDRESS_1,
        ORACLE_ADDRESS_2,
        ORACLE_ADDRESS_3,
        ORACLE_ADDRESS_4,
    ];

    for oracle in oracles {
        world
            .tx()
            .from(oracle)
            .to(price_aggregator_sc)
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

/// Setup template liquidity pool
pub fn setup_template_liquidity_pool(world: &mut ScenarioWorld) -> ManagedAddress<StaticApi> {
    world
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
        .run()
}

/// Setup template liquidity pool
pub fn setup_accumulator(world: &mut ScenarioWorld) -> ManagedAddress<StaticApi> {
    let accumulator_sc = world
        .tx()
        .from(OWNER_ADDRESS)
        .typed(proxy_accumulator::AccumulatorProxy)
        .init(
            ManagedAddress::zero(),
            BigUint::from(1_000u64),
            BigUint::from(3_000u64),
            TokenIdentifier::<StaticApi>::from(XEGLD_TOKEN.to_token_identifier()),
            TokenIdentifier::<StaticApi>::from(USDC_TOKEN.to_token_identifier()),
            ManagedAddress::zero(),
        )
        .code(ACCUMULATOR_PATH)
        .returns(ReturnsNewManagedAddress)
        .run();

    accumulator_sc
}

/// Create e-mode category
pub fn create_e_mode_category(world: &mut ScenarioWorld, lending_sc: &ManagedAddress<StaticApi>) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .add_e_mode_category(
            BigUint::from(LTV),
            BigUint::from(E_MODE_LIQ_THRESHOLD),
            BigUint::from(E_MODE_LIQ_BONUS),
        )
        .returns(ReturnsResult)
        .run();
}

/// Add asset to e-mode category
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

pub fn multiply(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    e_mode_category: u8,
    collateral_token: EgldOrEsdtTokenIdentifier<StaticApi>,
    debt_to_flash_loan: BigUint<StaticApi>,
    debt_token: EgldOrEsdtTokenIdentifier<StaticApi>,
    mode: PositionMode,
    steps: ManagedArgBuffer<StaticApi>,
    steps_payment: OptionalValue<ManagedArgBuffer<StaticApi>>,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .multiply(
            e_mode_category,
            collateral_token,
            debt_to_flash_loan,
            debt_token,
            mode,
            steps,
            match steps_payment.into_option() {
                Some(payment) => OptionalValue::Some(payment),
                None => OptionalValue::None,
            },
        )
        .returns(ReturnsResult)
        .run();
}

pub fn swap_collateral(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    current_collateral: EgldOrEsdtTokenIdentifier<StaticApi>,
    from_amount: BigUint<StaticApi>,
    new_collateral: EgldOrEsdtTokenIdentifier<StaticApi>,
    steps_payment: ManagedArgBuffer<StaticApi>,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .swap_collateral(
            current_collateral,
            from_amount,
            new_collateral,
            steps_payment,
        )
        .returns(ReturnsResult)
        .run();
}

pub fn swap_debt(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    existing_debt: EgldOrEsdtTokenIdentifier<StaticApi>,
    new_debt_amount: BigUint<StaticApi>,
    new_debt_token: EgldOrEsdtTokenIdentifier<StaticApi>,
    steps_payment: ManagedArgBuffer<StaticApi>,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .swap_debt(
            existing_debt,
            new_debt_amount,
            new_debt_token,
            steps_payment,
        )
        .returns(ReturnsResult)
        .run();
}

pub fn repay_debt_with_collateral(
    world: &mut ScenarioWorld,
    lending_sc: &ManagedAddress<StaticApi>,
    from_token: EgldOrEsdtTokenIdentifier<StaticApi>,
    from_amount: BigUint<StaticApi>,
    to_token: EgldOrEsdtTokenIdentifier<StaticApi>,
    close_position: bool,
    steps_payment: OptionalValue<ManagedArgBuffer<StaticApi>>,
) {
    world
        .tx()
        .from(OWNER_ADDRESS)
        .to(lending_sc)
        .typed(proxy_lending_pool::ControllerProxy)
        .repay_debt_with_collateral(
            from_token,
            from_amount,
            to_token,
            close_position,
            match steps_payment.into_option() {
                Some(payment) => OptionalValue::Some(payment),
                None => OptionalValue::None,
            },
        )
        .returns(ReturnsResult)
        .run();
}

/// Setup market
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
            config.config.loan_to_value_bps.into_raw_units(),
            config.config.liquidation_threshold_bps.into_raw_units(),
            config.config.liquidation_bonus_bps.into_raw_units(),
            config.config.liquidation_fees_bps.into_raw_units(),
            config.config.is_collateralizable,
            config.config.is_borrowable,
            config.config.is_isolated_asset,
            config
                .config
                .isolation_debt_ceiling_usd_wad
                .into_raw_units(),
            config.config.flashloan_fee_bps.into_raw_units(),
            config.config.is_siloed_borrowing,
            config.config.is_flashloanable,
            config.config.isolation_borrow_enabled,
            config.asset_decimals,
            config.config.borrow_cap_wad.unwrap_or(BigUint::zero()),
            config.config.supply_cap_wad.unwrap_or(BigUint::zero()),
        )
        .returns(ReturnsResult)
        .run();

    market_address
}

// ============================================
// HELPER FUNCTIONS FOR ACCOUNT SETUP
// ============================================

/// Setup accounts for testing
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
            DAI_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(DAI_DECIMALS as u32),
        )
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
            LXOXNO_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(LXOXNO_DECIMALS as u32),
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
            DAI_TOKEN,
            BigUint::from(100000000u64) * BigUint::from(10u64).pow(DAI_DECIMALS as u32),
        )
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
            LXOXNO_TOKEN,
            BigUint::from(10000u64) * BigUint::from(10u64).pow(LXOXNO_DECIMALS as u32),
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

/// Setup single account
pub fn setup_account(state: &mut LendingPoolTestState, account: TestAddress) {
    state
        .world
        .account(account)
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

/// Setup owner account
pub fn setup_owner(world: &mut ScenarioWorld) {
    world
        .account(OWNER_ADDRESS)
        .nonce(1)
        .balance(BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32))
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

/// Setup flasher account
pub fn setup_flasher(world: &mut ScenarioWorld, flash: ManagedAddress<StaticApi>) {
    world.set_esdt_balance(
        flash.clone(),
        EGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
    world.set_esdt_balance(
        flash,
        USDC_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
}

pub fn setup_swap_mock_owner(world: &mut ScenarioWorld, swap_mock: ManagedAddress<StaticApi>) {
    world.set_egld_balance(swap_mock.clone(), BigUint::from(10000000000000u64));
    world.set_esdt_balance(
        swap_mock.clone(),
        EGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(EGLD_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        XOXNO_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(XOXNO_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        ISOLATED_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(ISOLATED_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        CAPPED_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(CAPPED_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        SILOED_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(SILOED_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        XEGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(XEGLD_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        SEGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(SEGLD_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock.clone(),
        LEGLD_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(LEGLD_DECIMALS as u32),
    );
    world.set_esdt_balance(
        swap_mock,
        USDC_TOKEN.as_bytes(),
        BigUint::from(100000000u64) * BigUint::from(10u64).pow(USDC_DECIMALS as u32),
    );
}

// ============================================
// HELPER FUNCTIONS FOR MARKET OPERATIONS
// ============================================

impl LendingPoolTestState {
    /// Change the blockchain timestamp
    pub fn change_timestamp(&mut self, timestamp: u64) {
        self.world.current_block().block_timestamp(timestamp);
        let aggregator = &self.price_aggregator_sc;
        submit_price(
            &mut self.world,
            aggregator,
            EGLD_TICKER,
            EGLD_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            SEGLD_TICKER,
            SEGLD_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            LEGLD_TICKER,
            LEGLD_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            USDC_TICKER,
            USDC_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            XEGLD_TICKER,
            XEGLD_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            ISOLATED_TICKER,
            ISOLATED_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            SILOED_TICKER,
            SILOED_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            CAPPED_TICKER,
            CAPPED_PRICE_IN_DOLLARS,
            timestamp,
        );
        submit_price(
            &mut self.world,
            aggregator,
            XOXNO_TICKER,
            XOXNO_PRICE_IN_DOLLARS,
            timestamp,
        );
    }

    // Price aggregator operations
    pub fn change_price_denominated(
        &mut self,
        from: &[u8],
        price: BigUint<StaticApi>,
        timestamp: u64,
    ) {
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
                    price.clone(),
                )
                .run();
        }
    }
    // Price aggregator operations
    pub fn change_price(&mut self, from: &[u8], price: u64, timestamp: u64) {
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
    /// Get market utilization rate
    pub fn market_utilization(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .capital_utilisation()
            .returns(ReturnsResult)
            .run()
    }
    /// Get market borrow rate
    pub fn market_borrow_rate(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .borrow_rate()
            .returns(ReturnsResult)
            .run()
    }

    /// Get market supply rate
    pub fn market_supply_rate(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .deposit_rate()
            .returns(ReturnsResult)
            .run()
    }

    /// Get market reserves
    pub fn market_reserves(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .reserves()
            .returns(ReturnsResult)
            .run()
    }

    /// Get market revenue
    pub fn market_revenue(
        &mut self,
        market_address: ManagedAddress<StaticApi>,
    ) -> ManagedDecimal<StaticApi, NumDecimals> {
        self.world
            .query()
            .to(market_address)
            .typed(proxy_liquidity_pool::LiquidityPoolProxy)
            .protocol_revenue()
            .returns(ReturnsResult)
            .run()
    }
}

// ============================================
// TEST ASSERTION HELPERS
// ============================================

/// Returns the on-chain base units for a human-readable amount.
pub fn scaled_amount(amount: u128, decimals: usize) -> BigUint<StaticApi> {
    BigUint::from(amount) * BigUint::from(10u64).pow(decimals as u32)
}

/// Returns `true` when the supplied and borrowed values differ by at most `tolerance`.
fn raw_diff_within(
    first: &BigUint<StaticApi>,
    second: &BigUint<StaticApi>,
    tolerance: &BigUint<StaticApi>,
) -> bool {
    if first >= second {
        (first.clone() - second).le(tolerance)
    } else {
        (second.clone() - first).le(tolerance)
    }
}

impl LendingPoolTestState {
    /// Assert helper to verify the exact borrow balance stored on-chain.
    pub fn assert_borrow_raw_eq(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
        expected_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .borrow_amount_for_token(account_position, token_id.clone())
            .into_raw_units()
            .clone();

        assert_eq!(actual_raw, expected_raw, "{}", context,);
    }

    /// Assert helper to verify the collateral balance stored on-chain.
    pub fn assert_collateral_raw_eq(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
        expected_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .collateral_amount_for_token(account_position, token_id.clone())
            .into_raw_units()
            .clone();

        assert_eq!(actual_raw, expected_raw, "{}", context,);
    }

    /// Assert helper to ensure the health factor stays above the supplied threshold (in RAY units).
    pub fn assert_health_factor_at_least(&mut self, account_position: u64, minimum_ray: u128) {
        let actual_raw = self
            .account_health_factor(account_position)
            .into_raw_units()
            .clone();

        assert!(
            actual_raw >= BigUint::from(minimum_ray),
            "health factor below safety threshold: expected >= {minimum_ray}, got {actual_raw:?}",
        );
    }

    /// Assert helper for total borrow across all tokens (denominated in EGLD value, RAY precision).
    pub fn assert_total_borrow_raw_eq(
        &mut self,
        account_position: u64,
        expected_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .total_borrow_in_egld(account_position)
            .into_raw_units()
            .clone();

        assert_eq!(actual_raw, expected_raw, "{}", context);
    }

    /// Assert helper allowing tolerance when comparing total borrow in EGLD with RAY precision.
    pub fn assert_total_borrow_raw_within(
        &mut self,
        account_position: u64,
        expected_raw: BigUint<StaticApi>,
        tolerance_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .total_borrow_in_egld(account_position)
            .into_raw_units()
            .clone();

        assert!(
            raw_diff_within(&actual_raw, &expected_raw, &tolerance_raw),
            "{} | expected {:?} (±{:?}) got {:?}",
            context,
            expected_raw,
            tolerance_raw,
            actual_raw,
        );
    }

    /// Assert helper for total collateral across all markets (denominated in EGLD value, RAY precision).
    pub fn assert_total_collateral_raw_eq(
        &mut self,
        account_position: u64,
        expected_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .total_collateral_in_egld(account_position)
            .into_raw_units()
            .clone();

        assert_eq!(actual_raw, expected_raw, "{}", context);
    }

    /// Assert helper allowing tolerance when comparing total collateral in EGLD with RAY precision.
    pub fn assert_total_collateral_raw_within(
        &mut self,
        account_position: u64,
        expected_raw: BigUint<StaticApi>,
        tolerance_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .total_collateral_in_egld(account_position)
            .into_raw_units()
            .clone();

        assert!(
            raw_diff_within(&actual_raw, &expected_raw, &tolerance_raw),
            "{} | expected {:?} (±{:?}) got {:?}",
            context,
            expected_raw,
            tolerance_raw,
            actual_raw,
        );
    }

    /// Assert helper that allows a tolerance (in raw units) for rounding-sensitive flows.
    pub fn assert_borrow_raw_within(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
        expected_raw: BigUint<StaticApi>,
        tolerance_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .borrow_amount_for_token(account_position, token_id.clone())
            .into_raw_units()
            .clone();

        assert!(
            raw_diff_within(&actual_raw, &expected_raw, &tolerance_raw),
            "{} | expected {:?} (±{:?}) got {:?}",
            context,
            expected_raw,
            tolerance_raw,
            actual_raw,
        );
    }

    /// Assert helper that expects the account to have no collateral entry for the token.
    pub fn assert_no_collateral_entry(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
    ) {
        let expected_message = format!("Token not existing in the account {}", token_id.as_str());
        self.collateral_amount_for_token_non_existing(
            account_position,
            token_id.clone(),
            expected_message.as_bytes(),
        );
    }

    /// Assert helper that expects the account to have no borrow entry for the token.
    pub fn assert_no_borrow_entry(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
    ) {
        let expected_message = format!("Token not existing in the account {}", token_id.as_str());
        self.borrow_amount_for_token_non_existing(
            account_position,
            token_id.clone(),
            expected_message.as_bytes(),
        );
    }

    /// Assert helper that allows a tolerance (in raw units) for collateral balances.
    pub fn assert_collateral_raw_within(
        &mut self,
        account_position: u64,
        token_id: &TestTokenIdentifier,
        expected_raw: BigUint<StaticApi>,
        tolerance_raw: BigUint<StaticApi>,
        context: &str,
    ) {
        let actual_raw = self
            .collateral_amount_for_token(account_position, token_id.clone())
            .into_raw_units()
            .clone();

        assert!(
            raw_diff_within(&actual_raw, &expected_raw, &tolerance_raw),
            "{} | expected {:?} (±{:?}) got {:?}",
            context,
            expected_raw,
            tolerance_raw,
            actual_raw,
        );
    }
}
