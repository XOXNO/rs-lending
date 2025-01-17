multiversx_sc::imports!();

use crate::{
    contexts::base::StorageCache, helpers, lxoxno_proxy, oracle, position, proxy_pool, storage,
    utils, validation, xegld_proxy, ERROR_ASSET_NOT_BORROWABLE,
    ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
};
use common_constants::BP;
use common_events::{ExchangeSource, OracleType};

#[multiversx_sc::module]
pub trait MultiplyModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + validation::ValidationModule
    + position::PositionModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::math::MathsModule
{
    // e-mode 1
    // EGLD, xEGLD, xEGLD/EGLD LP
    // Send EGLD -> Stake for xEGLD -> Supply xEGLD (COLLATERAL) -> Borrow EGLD -> loop again
    // Send xEGLD -> Supply xEGLD (COLLATERAL) -> Borrow EGLD -> loop again
    #[payable("*")]
    #[endpoint]
    fn multiply(
        &self,
        leverage: &BigUint,
        e_mode_category: u8,
        collateral_token: &EgldOrEsdtTokenIdentifier,
    ) {
        let bp = BigUint::from(BP);
        let debt_payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();
        let e_mode = self.validate_e_mode_exists(e_mode_category);
        self.validate_not_depracated_e_mode(&e_mode);

        let target = &bp * 2u32 / 100u32 + &bp; // 1.02
        let reserves_factor = &bp / 5u64; // 20%
        self.require_asset_supported(collateral_token);

        let collateral_oracle = self.token_oracle(collateral_token).get();
        let mut collateral_config = self.asset_config(collateral_token).get();

        let mut debt_config = self.asset_config(&debt_payment.token_identifier).get();
        let asset_address = self.require_asset_supported(&debt_payment.token_identifier);

        let debt_market_sc = self.pools_map(&debt_payment.token_identifier).get();

        let max_l = self.calculate_max_leverage(
            &debt_payment.amount,
            &target,
            &e_mode,
            &debt_config,
            &self.get_total_reserves(debt_market_sc).get(),
            &reserves_factor,
        );

        require!(
            leverage <= &max_l,
            "The leverage is over the maximum allowed!"
        );

        let (account, nft_attributes) =
            self.enter(&caller, false, false, OptionalValue::Some(e_mode_category));
        let e_mode_id = nft_attributes.e_mode_category;
        // 4. Validate e-mode constraints first
        let collateral_emode_config = self.validate_token_of_emode(e_mode_id, &collateral_token);
        let debt_emode_config =
            self.validate_token_of_emode(e_mode_id, &debt_payment.token_identifier);

        self.validate_e_mode_not_isolated(&collateral_config, e_mode_id);
        self.validate_e_mode_not_isolated(&debt_config, e_mode_id);

        // 5. Update asset config if NFT has active e-mode
        self.update_asset_config_for_e_mode(
            &mut collateral_config,
            &e_mode,
            collateral_emode_config,
        );
        self.update_asset_config_for_e_mode(&mut debt_config, &e_mode, debt_emode_config);

        require!(
            collateral_config.can_be_collateral,
            ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL
        );

        require!(debt_config.can_be_borrowed, ERROR_ASSET_NOT_BORROWABLE);

        require!(
            collateral_oracle.token_type == OracleType::Derived,
            "Looping works only via LSD or LP tokens"
        );

        require!(
            debt_payment.token_identifier == collateral_oracle.first_token_id,
            "Payment has to be the underlaying LSD token"
        );

        let total_collateral = &debt_payment.amount * leverage;
        let flash_loan_amount = &total_collateral - &debt_payment.amount;
        let flash_fee = &flash_loan_amount * &debt_config.flash_loan_fee / &BigUint::from(BP);
        let total_borrowed = &flash_loan_amount + &flash_fee;

        let mut storage_cache = StorageCache::new(self);
        let feed = self.get_token_price(&debt_payment.token_identifier, &mut storage_cache);

        self.validate_borrow_cap(
            &debt_config,
            &total_borrowed,
            &debt_payment.token_identifier,
        );

        let latest_market_info = self
            .tx()
            .to(asset_address)
            .typed(proxy_pool::LiquidityPoolProxy)
            .internal_create_strategy(
                &debt_payment.token_identifier,
                &flash_loan_amount,
                &flash_fee,
                &feed.price,
            )
            .returns(ReturnsResult)
            .sync_call();

        let (borrow_index, timestamp) = latest_market_info;

        let mut borrow_position = self.get_or_create_borrow_position(
            account.token_nonce,
            &debt_config,
            debt_payment.token_identifier.clone(),
            false,
        );

        borrow_position.amount += &flash_loan_amount;
        borrow_position.accumulated_interest += &flash_fee;
        borrow_position.index = borrow_index;
        borrow_position.timestamp = timestamp;

        let mut borrow_positions = self.borrow_positions(account.token_nonce);

        self.update_position_event(
            &flash_loan_amount,
            &borrow_position,
            OptionalValue::Some(feed.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&nft_attributes),
        );
        borrow_positions.insert(debt_payment.token_identifier, borrow_position);

        // Convert the debt token to the LSD token

        let collateral_payment;
        if collateral_oracle.source == ExchangeSource::XEGLD {
            collateral_payment = self
                .tx()
                .to(&collateral_oracle.contract_address)
                .typed(xegld_proxy::LiquidStakingProxy)
                .delegate()
                .egld(total_collateral)
                .returns(ReturnsBackTransfersSingleESDT)
                .sync_call();
        } else if collateral_oracle.source == ExchangeSource::LXOXNO {
            collateral_payment = self
                .tx()
                .to(&collateral_oracle.contract_address)
                .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
                .delegate(OptionalValue::<ManagedAddress>::None)
                .egld_or_single_esdt(collateral_token, 0, &total_collateral)
                .returns(ReturnsBackTransfersSingleESDT)
                .sync_call();
        } else {
            panic!("Source not supported yet");
        }

        let feed_collateral = self.get_token_price(collateral_token, &mut storage_cache);

        self.update_supply_position(
            account.token_nonce,
            &EgldOrEsdtTokenPayment::new(
                EgldOrEsdtTokenIdentifier::esdt(collateral_payment.token_identifier.clone()),
                collateral_payment.token_nonce,
                collateral_payment.amount.clone(),
            ),
            &collateral_config,
            false,
            &feed_collateral,
            &caller,
            &nft_attributes,
        );

        // 4. Validate health factor after looping was created to verify integrity of healthy
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);
    }
}
