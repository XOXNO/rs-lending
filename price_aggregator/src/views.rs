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
        last_price: TimestampedPrice<Self::Api>,
    ) -> PriceFeed<Self::Api> {
        PriceFeed {
            round_id: last_price.round,
            from: token_pair.from,
            to: token_pair.to,
            timestamp: last_price.timestamp,
            price: last_price.price,
        }
    }

    #[view(latestRoundData)]
    fn latest_round_data(
        &self,
        pairs: MultiValueEncoded<TokenPair<Self::Api>>,
    ) -> MultiValueEncoded<PriceFeed<Self::Api>> {
        self.require_not_paused();

        let mut result = MultiValueEncoded::new();
        for token_pair in pairs {
            let round_values = self.rounds_new(&token_pair.from, &token_pair.to);
            if !round_values.is_empty() {
                result.push(self.make_price_feed(token_pair, round_values.get()));
            }
        }

        result
    }

    #[view(latestPriceFeed)]
    fn latest_price_feed(&self, from: ManagedBuffer, to: ManagedBuffer) -> PriceFeed<Self::Api> {
        require!(self.not_paused(), PAUSED_ERROR);

        let round_values = self.rounds_new(&from, &to);
        require!(!round_values.is_empty(), TOKEN_PAIR_NOT_FOUND_ERROR);

        let token_pair = TokenPair { from, to };
        
        self.make_price_feed(token_pair, round_values.get())
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
