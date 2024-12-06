use crate::{
    proxy_price_aggregator::{PriceAggregatorProxy, PriceFeed},
    storage, ERROR_PRICE_AGGREGATOR_NOT_SET, ERROR_TOKEN_TICKER_FETCH,
};

multiversx_sc::imports!();

const TOKEN_ID_SUFFIX_LEN: usize = 7; // "dash" + 6 random bytes
const DOLLAR_TICKER: &[u8] = b"USD";
#[multiversx_sc::module]
pub trait OracleModule: storage::LendingStorageModule {
    fn get_token_price_data(&self, token_id: &EgldOrEsdtTokenIdentifier) -> PriceFeed<Self::Api> {
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
            .sync_call();

        result
    }

    fn get_token_ticker(&self, token_id: &EgldOrEsdtTokenIdentifier) -> ManagedBuffer {
        if token_id.is_egld() {
            return ManagedBuffer::new_from_bytes(b"EGLD");
        }

        let as_buffer = token_id.clone().into_name();

        let ticker_start_index = 0;
        let ticker_end_index = as_buffer.len() - TOKEN_ID_SUFFIX_LEN;

        let result = as_buffer.copy_slice(ticker_start_index, ticker_end_index);

        match result {
            Some(r) => r,
            None => sc_panic!(ERROR_TOKEN_TICKER_FETCH),
        }
    }
}
