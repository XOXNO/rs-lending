#![allow(non_snake_case)]

use aggregator_mock::ProxyTrait as _;
use aggregator_mock::*;
use multiversx_sc_snippets::{
    multiversx_sc::{
        codec::multi_types::*,
        types::*,
    },
    env_logger,
    erdrs::wallet::Wallet,
    tokio, Interactor,
};
use multiversx_sc_scenario::scenario_model::*;
use multiversx_chain_vm::{
    bech32, scenario_format::interpret_trait::InterpreterContext, ContractInfo, DebugApi,
};


const GATEWAY: &str = multiversx_sdk::blockchain::DEVNET_GATEWAY;
const PEM: &str = "alice.pem";
const SC_ADDRESS: &str = "";

const SYSTEM_SC_BECH32: &str = "erd1qqqqqqqqqqqqqqqpqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqzllls8a5w6u";
const DEFAULT_ADDRESS_EXPR: &str = "0x0000000000000000000000000000000000000000000000000000000000000000";
const DEFAULT_GAS_LIMIT: u64 = 100_000_000;
const TOKEN_ISSUE_COST: u64 = 50_000_000_000_000_000;

type ContractType = ContractInfo<aggregator_mock::Proxy<DebugApi>>;

#[tokio::main]
async fn main() {
    env_logger::init();
    let _ = DebugApi::dummy();

    let mut args = std::env::args();
    let _ = args.next();
    let cmd = args.next().expect("at least one argument required");
    let mut state = State::new().await;
    match cmd.as_str() {
        "deploy" => state.deploy().await,
        "latestPriceFeedOptional" => state.latest_price_feed_optional().await,
        "setLatestPriceFeed" => state.set_latest_price_feed().await,
        _ => panic!("unknown command: {}", &cmd),
    }
}

struct State {
    interactor: Interactor,
    wallet_address: Address,
    contract: ContractType,
}

impl State {
    async fn new() -> Self {
        let mut interactor = Interactor::new(GATEWAY).await;
        let wallet_address = interactor.register_wallet(Wallet::from_pem_file(PEM).unwrap());
        let sc_addr_expr = if SC_ADDRESS == "" {
            DEFAULT_ADDRESS_EXPR.to_string()
        } else {
            "bech32:".to_string() + SC_ADDRESS
        };
        let contract = ContractType::new(sc_addr_expr);

        State {
            interactor,
            wallet_address,
            contract,
        }
    }

    async fn deploy(&mut self) {
        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_deploy(
                self.contract
                    .init()
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .code_metadata(CodeMetadata::all())
                    .contract_code("file:../output/aggregator-mock.wasm", &InterpreterContext::default())
                    .gas_limit(DEFAULT_GAS_LIMIT),
            )
            .await;

        let new_address = result.new_deployed_address();
        let new_address_bech32 = bech32::encode(&new_address);
        println!("new address: {}", new_address_bech32);
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn latest_price_feed_optional(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);

        let result_value: OptionalValue<MultiValue5<u32, ManagedBuffer<DebugApi>, ManagedBuffer<DebugApi>, BigUint<DebugApi>, u8>> = self
            .interactor
            .vm_query(self.contract.latest_price_feed_optional(from, to))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn set_latest_price_feed(&mut self) {
        let from = ManagedBuffer::new_from_bytes(&b""[..]);
        let to = ManagedBuffer::new_from_bytes(&b""[..]);
        let price = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .set_latest_price_feed(from, to, price)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

}
