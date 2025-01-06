multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{
    errors::*,
    structs::{PriceFeed, TimestampedPrice, TokenPair},
};

#[multiversx_sc::module]
pub trait ViewsModule:
    crate::storage::StorageModule + multiversx_sc_modules::pause::PauseModule
{
    fn make_price_feed(
        &self,
        token_pair: TokenPair<Self::Api>,
        round_values: VecMapper<TimestampedPrice<Self::Api>>,
    ) -> PriceFeed<Self::Api> {
        let round_id = round_values.len();
        let last_price = round_values.get(round_id);

        PriceFeed {
            round_id: round_id as u32,
            from: token_pair.from,
            to: token_pair.to,
            timestamp: last_price.timestamp,
            price: last_price.price,
            decimals: last_price.decimals,
        }
    }

    #[view(getPairDecimals)]
    fn get_pair_decimals(&self, from: &ManagedBuffer, to: &ManagedBuffer) -> u8 {
        self.pair_decimals(from, to)
            .get()
            .unwrap_or_else(|| sc_panic!(PAIR_DECIMALS_NOT_CONFIGURED_ERROR))
    }

    #[view(latestRoundData)]
    fn latest_round_data(
        &self,
        pairs: MultiValueEncoded<TokenPair<Self::Api>>,
    ) -> MultiValueEncoded<PriceFeed<Self::Api>> {
        self.require_not_paused();

        let mut result = MultiValueEncoded::new();
        for token_pair in pairs {
            let round_values = self.rounds(&token_pair.from, &token_pair.to);
            if round_values.len() > 0 {
                result.push(self.make_price_feed(token_pair, round_values));
            }
        }

        result
    }

    #[view(latestPriceFeed)]
    fn latest_price_feed(&self, from: ManagedBuffer, to: ManagedBuffer) -> PriceFeed<Self::Api> {
        require!(self.not_paused(), PAUSED_ERROR);

        let round_values = self.rounds(&from, &to);
        require!(!round_values.is_empty(), TOKEN_PAIR_NOT_FOUND_ERROR);

        let token_pair = TokenPair { from, to };
        let feed = self.make_price_feed(token_pair, round_values);
        feed
    }

    #[view(latestPriceFeedOptional)]
    fn latest_price_feed_optional(
        &self,
        from: ManagedBuffer,
        to: ManagedBuffer,
    ) -> OptionalValue<PriceFeed<Self::Api>> {
        Some(self.latest_price_feed(from, to)).into()
    }

    #[view(getOracles)]
    fn get_oracles(&self) -> MultiValueEncoded<ManagedAddress> {
        let mut result = MultiValueEncoded::new();
        for key in self.oracle_status().keys() {
            result.push(key);
        }
        result
    }
}
