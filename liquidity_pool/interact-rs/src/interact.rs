#![allow(non_snake_case)]

use liquidity_pool::ProxyTrait as _;
use liquidity_pool::*;
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

type ContractType = ContractInfo<liquidity_pool::Proxy<DebugApi>>;

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
        "getPoolAsset" => state.pool_asset().await,
        "getReserves" => state.reserves().await,
        "getSuppliedAmount" => state.supplied_amount().await,
        "getRewardsReserves" => state.rewards_reserves().await,
        "getLendToken" => state.lend_token().await,
        "borrowToken" => state.borrow_token().await,
        "getPoolParams" => state.pool_params().await,
        "getTotalBorrow" => state.borrowed_amount().await,
        "getLiquidationThreshold" => state.liquidation_threshold().await,
        "getBorrowIndex" => state.borrow_index().await,
        "getSupplyIndex" => state.supply_index().await,
        "borrowIndexLastUpdateRound" => state.borrow_index_last_update_round().await,
        "getAccountToken" => state.account_token().await,
        "getDebtNFT" => state.debt_nft_token().await,
        "getAccountPositions" => state.account_positions().await,
        "updateCollateralWithInterest" => state.update_collateral_with_interest().await,
        "updateBorrowsWithDebt" => state.update_borrows_with_debt().await,
        "addCollateral" => state.add_collateral().await,
        "borrow" => state.borrow().await,
        "borrowWithNFTs" => state.borrow_bulk_nfts().await,
        "remove_collateral" => state.remove_collateral().await,
        "repay" => state.repay().await,
        "repayNFTs" => state.repay_nfts().await,
        "sendTokens" => state.send_tokens().await,
        "getCapitalUtilisation" => state.get_capital_utilisation().await,
        "getTotalCapital" => state.get_total_capital().await,
        "getDebtInterest" => state.get_debt_interest().await,
        "getDepositRate" => state.get_deposit_rate().await,
        "getBorrowRate" => state.get_borrow_rate().await,
        "setPriceAggregatorAddress" => state.set_price_aggregator_address().await,
        "getAggregatorAddress" => state.price_aggregator_address().await,
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
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let r_base = BigUint::<DebugApi>::from(0u128);
        let r_slope1 = BigUint::<DebugApi>::from(0u128);
        let r_slope2 = BigUint::<DebugApi>::from(0u128);
        let u_optimal = BigUint::<DebugApi>::from(0u128);
        let reserve_factor = BigUint::<DebugApi>::from(0u128);
        let liquidation_threshold = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_deploy(
                self.contract
                    .init(asset, r_base, r_slope1, r_slope2, u_optimal, reserve_factor, liquidation_threshold)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .code_metadata(CodeMetadata::all())
                    .contract_code("file:../output/liquidity-pool.wasm", &InterpreterContext::default())
                    .gas_limit(DEFAULT_GAS_LIMIT),
            )
            .await;

        let new_address = result.new_deployed_address();
        let new_address_bech32 = bech32::encode(&new_address);
        println!("new address: {}", new_address_bech32);
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn pool_asset(&mut self) {
        let result_value: TokenIdentifier<DebugApi> = self
            .interactor
            .vm_query(self.contract.pool_asset())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn reserves(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.reserves())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn supplied_amount(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.supplied_amount())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn rewards_reserves(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.rewards_reserves())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn lend_token(&mut self) {
        let result_value: TokenIdentifier<DebugApi> = self
            .interactor
            .vm_query(self.contract.lend_token())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn borrow_token(&mut self) {
        let result_value: TokenIdentifier<DebugApi> = self
            .interactor
            .vm_query(self.contract.borrow_token())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn pool_params(&mut self) {
        let result_value: PoolParams<DebugApi> = self
            .interactor
            .vm_query(self.contract.pool_params())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn borrowed_amount(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.borrowed_amount())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn liquidation_threshold(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.liquidation_threshold())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn borrow_index(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.borrow_index())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn supply_index(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.supply_index())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn borrow_index_last_update_round(&mut self) {
        let result_value: u64 = self
            .interactor
            .vm_query(self.contract.borrow_index_last_update_round())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn account_token(&mut self) {
        let result_value: TokenIdentifier<DebugApi> = self
            .interactor
            .vm_query(self.contract.account_token())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn debt_nft_token(&mut self) {
        let result_value: TokenIdentifier<DebugApi> = self
            .interactor
            .vm_query(self.contract.debt_nft_token())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn account_positions(&mut self) {
        let result_value: MultiValueVec<u64> = self
            .interactor
            .vm_query(self.contract.account_positions())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn update_collateral_with_interest(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let deposit_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<DepositPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .update_collateral_with_interest(deposit_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn update_borrows_with_debt(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let borrow_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<BorrowPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .update_borrows_with_debt(borrow_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn add_collateral(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let deposit_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<DepositPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .add_collateral(deposit_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn borrow(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_amount = BigUint::<DebugApi>::from(0u128);
        let existing_borrow_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<BorrowPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .borrow(initial_caller, borrow_amount, existing_borrow_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn borrow_bulk_nfts(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_amount = BigUint::<DebugApi>::from(0u128);
        let existing_borrow_positions = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<ManagedVec<DebugApi, BorrowPosition<DebugApi>>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .borrow_bulk_nfts(initial_caller, borrow_amount, existing_borrow_positions)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn remove_collateral(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let amount = BigUint::<DebugApi>::from(0u128);
        let deposit_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<DepositPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .remove_collateral(initial_caller, amount, deposit_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn repay(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_position = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<BorrowPosition<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .repay(initial_caller, borrow_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn repay_nfts(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_positions = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<MultiValueVec<MultiValue2<EsdtTokenPayment<DebugApi>, BorrowPosition<DebugApi>>>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .repay_nfts(initial_caller, borrow_positions)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .esdt_transfer(token_id.to_vec(), token_nonce, token_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn send_tokens(&mut self) {
        let initial_caller = bech32::decode("");
        let payment_amount = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .send_tokens(initial_caller, payment_amount)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn get_capital_utilisation(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_capital_utilisation())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_total_capital(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_total_capital())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_debt_interest(&mut self) {
        let amount = BigUint::<DebugApi>::from(0u128);
        let initial_borrow_index = BigUint::<DebugApi>::from(0u128);

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_debt_interest(amount, initial_borrow_index))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_deposit_rate(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_deposit_rate())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_borrow_rate(&mut self) {
        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_borrow_rate())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn set_price_aggregator_address(&mut self) {
        let address = bech32::decode("");

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .set_price_aggregator_address(address)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn price_aggregator_address(&mut self) {
        let result_value: ManagedAddress<DebugApi> = self
            .interactor
            .vm_query(self.contract.price_aggregator_address())
            .await;

        println!("Result: {:?}", result_value);
    }

}
