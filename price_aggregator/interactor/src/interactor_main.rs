#![allow(non_snake_case)]

mod proxy;
mod config;

use config::Config;
use multiversx_sc_snippets::imports::*;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
};


const STATE_FILE: &str = "state.toml";

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut args = std::env::args();
    let _ = args.next();
    let cmd = args.next().expect("at least one argument required");
    let mut interact = ContractInteract::new().await;
    match cmd.as_str() {
        "deploy" => interact.deploy().await,
        "upgrade" => interact.upgrade().await,
        "addOracles" => interact.add_oracles().await,
        "removeOracles" => interact.remove_oracles().await,
        "submit" => interact.submit().await,
        "submitBatch" => interact.submit_batch().await,
        "latestRoundData" => interact.latest_round_data().await,
        "latestPriceFeed" => interact.latest_price_feed().await,
        "latestPriceFeedOptional" => interact.latest_price_feed_optional().await,
        "setSubmissionCount" => interact.set_submission_count().await,
        "getOracles" => interact.get_oracles().await,
        "setPairDecimals" => interact.set_pair_decimals().await,
        "getPairDecimals" => interact.get_pair_decimals().await,
        "submission_count" => interact.submission_count().await,
        "pause" => interact.pause_endpoint().await,
        "unpause" => interact.unpause_endpoint().await,
        "isPaused" => interact.paused_status().await,
        _ => panic!("unknown command: {}", &cmd),
    }
}


#[derive(Debug, Default, Serialize, Deserialize)]
struct State {
    contract_address: Option<Bech32Address>
}

impl State {
        // Deserializes state from file
        pub fn load_state() -> Self {
            if Path::new(STATE_FILE).exists() {
                let mut file = std::fs::File::open(STATE_FILE).unwrap();
                let mut content = String::new();
                file.read_to_string(&mut content).unwrap();
                toml::from_str(&content).unwrap()
            } else {
                Self::default()
            }
        }
    
        /// Sets the contract address
        pub fn set_address(&mut self, address: Bech32Address) {
            self.contract_address = Some(address);
        }
    
        /// Returns the contract address
        pub fn current_address(&self) -> &Bech32Address {
            self.contract_address
                .as_ref()
                .expect("no known contract, deploy first")
        }
    }
    
    impl Drop for State {
        // Serializes state to file
        fn drop(&mut self) {
            let mut file = std::fs::File::create(STATE_FILE).unwrap();
            file.write_all(toml::to_string(self).unwrap().as_bytes())
                .unwrap();
        }
    }

struct ContractInteract {
    interactor: Interactor,
    wallet_address: Address,
    contract_code: BytesValue,
    state: State
}

impl ContractInteract {
    async fn new() -> Self {
        let config = Config::new();
        let mut interactor = Interactor::new(config.gateway_uri(), config.use_chain_simulator()).await;
        interactor.set_current_dir_from_workspace("price_aggregator");

        let wallet_address = interactor.register_wallet(test_wallets::alice()).await;
        
        let contract_code = BytesValue::interpret_from(
            "mxsc:../output/price_aggregator.mxsc.json",
            &InterpreterContext::default(),
        );

        ContractInteract {
            interactor,
            wallet_address,
            contract_code,
            state: State::load_state()
        }
    }

    async fn deploy(&mut self) {
        let submission_count = 0u32;
        let oracles = MultiValueVec::from(vec![bech32::decode("")]);

        let new_address = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .init(submission_count, oracles)
            .code(&self.contract_code)
            .returns(ReturnsNewAddress)
            
            .run()
            .await;
        let new_address_bech32 = bech32::encode(&new_address);
        self.state
            .set_address(Bech32Address::from_bech32_string(new_address_bech32.clone()));

        println!("new address: {new_address_bech32}");
    }

    async fn upgrade(&mut self) {
        let response = self
            .interactor
            .tx()
            .to(self.state.current_address())
            .from(&self.wallet_address)
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .upgrade()
            .code(&self.contract_code)
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .returns(ReturnsNewAddress)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn add_oracles(&mut self) {
        let oracles = MultiValueVec::from(vec![bech32::decode("")]);

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .add_oracles(oracles)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn remove_oracles(&mut self) {
        let submission_count = 0u32;
        let oracles = MultiValueVec::from(vec![bech32::decode("")]);

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .remove_oracles(submission_count, oracles)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn submit(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);
        let submission_timestamp = 0u64;
        let price = BigUint::<StaticApi>::from(0u128);
        let decimals = 0u8;

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .submit(from, to, submission_timestamp, price, decimals)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn submit_batch(&mut self) {
        let submissions = MultiValueVec::from(vec![MultiValue5::<ManagedBuffer<StaticApi>, ManagedBuffer<StaticApi>, u64, BigUint<StaticApi>, u8>::from((ManagedBuffer::new_from_bytes(&b""[..]), ManagedBuffer::new_from_bytes(&b""[..]), 0u64, BigUint::<StaticApi>::from(0u128), 0u8))]);

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .submit_batch(submissions)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn latest_round_data(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .latest_round_data()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn latest_price_feed(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);

        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .latest_price_feed(from, to)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn latest_price_feed_optional(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);

        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .latest_price_feed_optional(from, to)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn set_submission_count(&mut self) {
        let submission_count = 0u32;

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .set_submission_count(submission_count)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn get_oracles(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .get_oracles()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn set_pair_decimals(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);
        let decimals = 0u8;

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .set_pair_decimals(from, to, decimals)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn get_pair_decimals(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);

        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .get_pair_decimals(from, to)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn submission_count(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .submission_count()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn pause_endpoint(&mut self) {
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .pause_endpoint()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn unpause_endpoint(&mut self) {
        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::PriceAggregatorProxy)
            .unpause_endpoint()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn paused_status(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::PriceAggregatorProxy)
            .paused_status()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

}
