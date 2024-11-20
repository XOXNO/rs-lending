multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{
    math, storage, ERROR_ASSET_NOT_SUPPORTED, ERROR_LOAN_TO_VALUE_ZERO, ERROR_NO_COLLATERAL_TOKEN,
    ERROR_NO_LIQUIDATION_BONUS, ERROR_NO_LOAN_TO_VALUE, ERROR_PRICE_AGGREGATOR_NOT_SET,
    ERROR_TOKEN_TICKER_FETCH,
};

use crate::proxy_price_aggregator::{PriceAggregatorProxy, PriceFeed};
use common_structs::*;

const TOKEN_ID_SUFFIX_LEN: usize = 7; // "dash" + 6 random bytes
const DOLLAR_TICKER: &[u8] = b"USD";

#[multiversx_sc::module]
pub trait LendingUtilsModule: math::LendingMathModule + storage::LendingStorageModule {
    fn get_token_price_data(&self, token_id: &TokenIdentifier) -> PriceFeed<Self::Api> {
        let from_ticker = self.get_token_ticker(token_id);
        let price_aggregator_address = self.price_aggregator_address();

        require!(
            !price_aggregator_address.is_empty(),
            ERROR_PRICE_AGGREGATOR_NOT_SET
        );

        let result = self
            .tx()
            .to(self.price_aggregator_address().get())
            .typed(PriceAggregatorProxy)
            .latest_price_feed(from_ticker, ManagedBuffer::new_from_bytes(DOLLAR_TICKER))
            .returns(ReturnsResult)
            .sync_call_readonly();

        result
    }

    fn get_token_ticker(&self, token_id: &TokenIdentifier) -> ManagedBuffer {
        let as_buffer = token_id.clone().into_managed_buffer();
        let ticker_start_index = 0;
        let ticker_end_index = as_buffer.len() - TOKEN_ID_SUFFIX_LEN;

        let result = as_buffer.copy_slice(ticker_start_index, ticker_end_index);

        match result {
            Some(r) => r,
            None => sc_panic!(ERROR_TOKEN_TICKER_FETCH),
        }
    }

    fn get_existing_or_new_deposit_position_for_token(
        &self,
        account_position: u64,
        token_id: TokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        match self.deposit_positions(account_position).get(&token_id) {
            Some(dp) => {
                self.deposit_positions(account_position).remove(&token_id);
                dp
            }
            None => AccountPosition::new(
                AccountPositionType::Deposit,
                token_id,
                BigUint::zero(),
                account_position,
                self.blockchain().get_block_round(),
                BigUint::from(BP),
            ),
        }
    }

    fn get_existing_or_new_borrow_position_for_token(
        &self,
        account_position: u64,
        token_id: TokenIdentifier,
    ) -> AccountPosition<Self::Api> {
        match self.borrow_positions(account_position).get(&token_id) {
            Some(bp) => bp,
            None => AccountPosition::new(
                AccountPositionType::Borrow,
                token_id,
                BigUint::zero(),
                account_position,
                self.blockchain().get_block_round(),
                BigUint::from(BP),
            ),
        }
    }

    #[inline]
    #[view(getCollateralAmountForToken)]
    fn get_collateral_amount_for_token(
        &self,
        account_position: u64,
        token_id: &TokenIdentifier,
    ) -> BigUint {
        match self.deposit_positions(account_position).get(token_id) {
            Some(dp) => dp.amount,
            None => BigUint::zero(),
        }
    }

    #[inline]
    #[view(getTotalCollateralAvailable)]
    fn get_total_collateral_in_dollars(&self, account_position: u64) -> BigUint {
        let mut deposited_amount_in_dollars = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            let dp_data = self.get_token_price_data(&dp.token_id);
            deposited_amount_in_dollars += dp.amount * dp_data.price;
        }

        deposited_amount_in_dollars
    }

    #[inline]
    #[view(getWeightedCollateralInDollars)]
    fn get_weighted_collateral_in_dollars(&self, account_position: u64) -> BigUint {
        let mut weighted_collateral_in_dollars = BigUint::zero();
        let deposit_positions = self.deposit_positions(account_position);

        for dp in deposit_positions.values() {
            let token_ltv = self.get_loan_to_value_exists_and_non_zero(&dp.token_id);
            let dp_data = self.get_token_price_data(&dp.token_id);
            let position_value_in_dollars = dp.amount.clone() * dp_data.price;
            weighted_collateral_in_dollars +=
                position_value_in_dollars * token_ltv / BigUint::from(BP);
        }

        weighted_collateral_in_dollars
    }

    #[view(getTotalBorrowInDollars)]
    fn get_total_borrow_in_dollars(&self, account_position: u64) -> BigUint {
        let mut total_borrow_in_dollars = BigUint::zero();
        let borrow_positions = self.borrow_positions(account_position);

        for bp in borrow_positions.values() {
            let bp_data = self.get_token_price_data(&bp.token_id);
            total_borrow_in_dollars += bp.amount * bp_data.price;
        }

        total_borrow_in_dollars
    }

    fn get_liquidation_bonus_non_zero(&self, token_id: &TokenIdentifier) -> BigUint {
        let liq_bonus = self.asset_liquidation_bonus(token_id).get();
        require!(liq_bonus > 0, ERROR_NO_LIQUIDATION_BONUS);

        liq_bonus
    }

    fn get_loan_to_value_exists_and_non_zero(&self, token_id: &TokenIdentifier) -> BigUint {
        require!(
            !self.asset_loan_to_value(token_id).is_empty(),
            ERROR_NO_LOAN_TO_VALUE
        );

        let loan_to_value = self.asset_loan_to_value(token_id).get();
        require!(loan_to_value > 0, ERROR_LOAN_TO_VALUE_ZERO);

        loan_to_value
    }

    fn require_asset_supported(&self, asset: &TokenIdentifier) {
        require!(!self.pools_map(asset).is_empty(), ERROR_ASSET_NOT_SUPPORTED);
    }

    fn compute_amount_in_tokens(
        &self,
        liquidatee_account_nonce: u64,
        token_to_liquidate: &TokenIdentifier, // collateral token of the debt position
        amount_to_return_to_liquidator_in_dollars: BigUint, // amount to return to the liquidator with bonus
    ) -> BigUint {
        require!(
            self.deposit_positions(liquidatee_account_nonce)
                .contains_key(token_to_liquidate),
            ERROR_NO_COLLATERAL_TOKEN
        );

        // Take the USD price of the token that the liquidator will receive
        let token_data = self.get_token_price_data(token_to_liquidate);
        // Convert the amount to return to the liquidator with bonus to the token amount
        (&amount_to_return_to_liquidator_in_dollars * BP / &token_data.price)
            * BigUint::from(10u64).pow(token_data.decimals as u32)
            / BP
    }
}
