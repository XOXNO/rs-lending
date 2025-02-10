multiversx_sc::imports!();

use crate::{
    contexts::base::StorageCache, helpers, oracle, positions, proxy_pool, storage, utils,
    validation, ERROR_ASSET_NOT_BORROWABLE, ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL,
};
use common_constants::BP;

#[multiversx_sc::module]
pub trait MultiplyModule:
    storage::LendingStorageModule
    + oracle::OracleModule
    + validation::ValidationModule
    + utils::LendingUtilsModule
    + common_events::EventsModule
    + helpers::math::MathsModule
    + helpers::strategies::StrategiesModule
    + positions::account::PositionAccountModule
    + positions::deposit::PositionDepositModule
    + positions::borrow::PositionBorrowModule
    + positions::withdraw::PositionWithdrawModule
    + positions::emode::EModeModule
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
        debt_token: &EgldOrEsdtTokenIdentifier,
    ) {
        let bp = BigUint::from(BP);
        let payment = self.call_value().egld_or_single_esdt();
        let caller = self.blockchain().get_caller();
        let e_mode = self.validate_e_mode_exists(e_mode_category);
        self.validate_not_depracated_e_mode(&e_mode);

        let mut storage_cache = StorageCache::new(self);

        let target = &bp * 2u32 / 100u32 + &bp; // 1.02
        let reserves_factor = &bp / 5u64; // 20%

        let collateral_market_sc = self.require_asset_supported(collateral_token);
        let debt_market_sc = self.require_asset_supported(debt_token);

        let collateral_oracle = self.token_oracle(collateral_token).get();
        let debt_oracle = self.token_oracle(debt_token).get();
        let payment_oracle = self.token_oracle(&payment.token_identifier).get();

        let mut collateral_config = self.asset_config(collateral_token).get();
        let mut debt_config = self.asset_config(debt_token).get();

        let collateral_price_feed = self.get_token_price(collateral_token, &mut storage_cache);
        let debt_price_feed = self.get_token_price(debt_token, &mut storage_cache);

        // let max_l = self.calculate_max_leverage(
        //     &debt_payment.amount,
        //     &target,
        //     &e_mode,
        //     &debt_config,
        //     &self.get_total_reserves(debt_market_sc).get(),
        //     &reserves_factor,
        // );

        // require!(
        //     leverage <= &max_l,
        //     "The leverage is over the maximum allowed: {}!",
        //     max_l
        // );

        let (account, nft_attributes) =
            self.enter(&caller, false, false, OptionalValue::Some(e_mode_category));

        let e_mode_id = nft_attributes.e_mode_category;
        // 4. Validate e-mode constraints first
        let collateral_emode_config = self.validate_token_of_emode(e_mode_id, &collateral_token);
        let debt_emode_config = self.validate_token_of_emode(e_mode_id, &debt_token);

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

        let initial_collateral = self.process_payment_to_collateral(
            &payment,
            &payment_oracle,
            collateral_token,
            &collateral_oracle,
        );

        let initial_egld_collateral =
            self.get_token_amount_in_egld_raw(&initial_collateral.amount, &collateral_price_feed);
        let final_strategy_collateral = &initial_egld_collateral * leverage / &bp;
        let required_collateral = &final_strategy_collateral - &initial_egld_collateral;

        let debt_amount_to_flash_loan =
            self.compute_amount_in_tokens(&required_collateral, &debt_price_feed);

        let flash_fee = &debt_amount_to_flash_loan * &debt_config.flash_loan_fee / &bp;
        let total_borrowed = &debt_amount_to_flash_loan + &flash_fee;

        self.validate_borrow_cap(&debt_config, &total_borrowed, debt_token);

        let (borrow_index, timestamp) = self
            .tx()
            .to(debt_market_sc)
            .typed(proxy_pool::LiquidityPoolProxy)
            .internal_create_strategy(
                debt_token,
                &debt_amount_to_flash_loan,
                &flash_fee,
                &debt_price_feed.price,
            )
            .returns(ReturnsResult)
            .sync_call();

        let mut borrow_position = self.get_or_create_borrow_position(
            account.token_nonce,
            &debt_config,
            debt_token,
            false,
        );

        borrow_position.amount += &debt_amount_to_flash_loan;
        borrow_position.accumulated_interest += &flash_fee;
        borrow_position.index = borrow_index;
        borrow_position.timestamp = timestamp;

        let mut borrow_positions = self.borrow_positions(account.token_nonce);

        self.update_position_event(
            &debt_amount_to_flash_loan,
            &borrow_position,
            OptionalValue::Some(debt_price_feed.price),
            OptionalValue::Some(&caller),
            OptionalValue::Some(&nft_attributes),
        );

        borrow_positions.insert(debt_token.clone(), borrow_position);

        // Convert the debt token to the LSD token
        let final_collateral = self.process_flash_loan_to_collateral(
            &debt_token,
            &debt_amount_to_flash_loan,
            collateral_token,
            &initial_collateral.amount,
            &collateral_oracle,
            &debt_oracle,
        );

        // sc_panic!(
        //     "Final collateral {}, Initial {}, Borrowed: {}",
        //     final_collateral.amount,
        //     initial_collateral.amount,
        //     flash_borrow.amount
        // );

        self.update_supply_position(
            account.token_nonce,
            &final_collateral,
            &collateral_config,
            false,
            &caller,
            &nft_attributes,
            &mut storage_cache,
        );

        // 4. Validate health factor after looping was created to verify integrity of healthy
        self.validate_withdraw_health_factor(account.token_nonce, false, &mut storage_cache, None);
    }
}
