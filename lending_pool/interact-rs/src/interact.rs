#![allow(non_snake_case)]

use lending_pool::ProxyTrait as _;
use lending_pool::*;
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
use price_aggregator_proxy::ProxyTrait;
type ContractType = ContractInfo<lending_pool::Proxy<DebugApi>>;

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
        "registerAccountToken" => state.register_account_token().await,
        "registerDebtNFTToken" => state.register_debt_token().await,
        "enterMarket" => state.enter_market().await,
        "exitMarket" => state.exit_market().await,
        "addCollateral" => state.add_collateral().await,
        "removeCollateral" => state.remove_collateral().await,
        "borrow" => state.borrow().await,
        "borrowWithNFTs" => state.borrow_with_nfts().await,
        "repay" => state.repay().await,
        "repayNFT" => state.repay_nft_debt().await,
        "liquidate" => state.liquidate().await,
        "updateCollateralWithInterest" => state.update_collateral_with_interest().await,
        "updateBorrowsWithDebt" => state.update_borrows_with_debt().await,
        "getLiqPoolTemplateAddress" => state.liq_pool_template_address().await,
        "createLiquidityPool" => state.create_liquidity_pool().await,
        "upgradeLiquidityPool" => state.upgrade_liquidity_pool().await,
        "setAggregator" => state.set_aggregator().await,
        "setAssetLoanToValue" => state.set_asset_loan_to_value().await,
        "setAssetLiquidationBonus" => state.set_asset_liquidation_bonus().await,
        "addCollection" => state.add_collection().await,
        "getPoolAddress" => state.get_pool_address().await,
        "getPoolAllowed" => state.pools_allowed().await,
        "getAssetLoanToValue" => state.asset_loan_to_value().await,
        "getAssetLiquidationBonus" => state.asset_liquidation_bonus().await,
        "getAccountToken" => state.account_token().await,
        "getDebtNFT" => state.debt_nft_token().await,
        "getAccountPositions" => state.account_positions().await,
        "getDepositPositions" => state.deposit_positions().await,
        "getBorrowPositions" => state.borrow_positions().await,
        "getNFTBorrowPositions" => state.nft_borrow_positions().await,
        "getCollections" => state.collections().await,
        "getCollectionParam" => state.collection_params().await,
        "getCollateralAmountForToken" => state.get_collateral_amount_for_token().await,
        "getTotalCollateralAvailable" => state.get_total_collateral_in_dollars().await,
        "getTotalBorrowInDollars" => state.get_total_borrow_in_dollars().await,
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
        let lp_template_address = bech32::decode("");

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_deploy(
                self.contract
                    .init(lp_template_address)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .code_metadata(CodeMetadata::all())
                    .contract_code("file:../output/lending-pool.wasm", &InterpreterContext::default())
                    .gas_limit(DEFAULT_GAS_LIMIT),
            )
            .await;

        let new_address = result.new_deployed_address();
        let new_address_bech32 = bech32::encode(&new_address);
        println!("new address: {}", new_address_bech32);
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn register_account_token(&mut self) {
        let egld_amount = BigUint::<DebugApi>::from(0u128);

        let token_name = ManagedBuffer::new_from_bytes(&b""[..]);
        let ticker = ManagedBuffer::new_from_bytes(&b""[..]);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .register_account_token(token_name, ticker)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .egld_value(egld_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn register_debt_token(&mut self) {
        let egld_amount = BigUint::<DebugApi>::from(0u128);

        let token_name = ManagedBuffer::new_from_bytes(&b""[..]);
        let ticker = ManagedBuffer::new_from_bytes(&b""[..]);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .register_debt_token(token_name, ticker)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
            .egld_value(egld_amount)

                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn enter_market(&mut self) {
        let result: multiversx_sc_snippets::InteractorResult<EsdtTokenPayment<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .enter_market()
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn exit_market(&mut self) {
        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .exit_market()
                    .into_blockchain_call()
                    .from(&self.wallet_address)
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

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .add_collateral()
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

        let withdraw_token_id = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let amount = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .remove_collateral(withdraw_token_id, amount)
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

        let asset_to_borrow = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let amount = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .borrow(asset_to_borrow, amount)
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

    async fn borrow_with_nfts(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let asset_to_borrow = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let amount = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .borrow_with_nfts(asset_to_borrow, amount)
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

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .repay()
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

    async fn repay_nft_debt(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<ManagedVec<DebugApi, EsdtTokenPayment<DebugApi>>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .repay_nft_debt()
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

    async fn liquidate(&mut self) {
        let token_id = b"";
        let token_nonce = 0u64;
        let token_amount = BigUint::<DebugApi>::from(0u128);

        let liquidatee_account_nonce = 0u64;
        let liquidation_threshold = BigUint::<DebugApi>::from(0u128);
        let token_to_liquidate = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .liquidate(liquidatee_account_nonce, liquidation_threshold, token_to_liquidate)
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

    async fn update_collateral_with_interest(&mut self) {
        let account_position = 0u64;

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .update_collateral_with_interest(account_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn update_borrows_with_debt(&mut self) {
        let account_position = 0u64;

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .update_borrows_with_debt(account_position)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn liq_pool_template_address(&mut self) {
        let result_value: ManagedAddress<DebugApi> = self
            .interactor
            .vm_query(self.contract.liq_pool_template_address())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn create_liquidity_pool(&mut self) {
        let base_asset = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let r_base = BigUint::<DebugApi>::from(0u128);
        let r_slope1 = BigUint::<DebugApi>::from(0u128);
        let r_slope2 = BigUint::<DebugApi>::from(0u128);
        let u_optimal = BigUint::<DebugApi>::from(0u128);
        let reserve_factor = BigUint::<DebugApi>::from(0u128);
        let liquidation_threshold = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<ManagedAddress<DebugApi>> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .create_liquidity_pool(base_asset, r_base, r_slope1, r_slope2, u_optimal, reserve_factor, liquidation_threshold)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn upgrade_liquidity_pool(&mut self) {
        let base_asset = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let r_base = BigUint::<DebugApi>::from(0u128);
        let r_slope1 = BigUint::<DebugApi>::from(0u128);
        let r_slope2 = BigUint::<DebugApi>::from(0u128);
        let u_optimal = BigUint::<DebugApi>::from(0u128);
        let reserve_factor = BigUint::<DebugApi>::from(0u128);
        let liquidation_threshold = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .upgrade_liquidity_pool(base_asset, r_base, r_slope1, r_slope2, u_optimal, reserve_factor, liquidation_threshold)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn set_aggregator(&mut self) {
        let pool_asset_id = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let aggregator = bech32::decode("");

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .set_aggregator(pool_asset_id, aggregator)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn set_asset_loan_to_value(&mut self) {
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let loan_to_value = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .set_asset_loan_to_value(asset, loan_to_value)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn set_asset_liquidation_bonus(&mut self) {
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);
        let liq_bonus = BigUint::<DebugApi>::from(0u128);

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .set_asset_liquidation_bonus(asset, liq_bonus)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn add_collection(&mut self) {
        let params = PlaceholderInput;

        let result: multiversx_sc_snippets::InteractorResult<()> = self
            .interactor
            .sc_call_get_result(
                self.contract
                    .add_collection(params)
                    .into_blockchain_call()
                    .from(&self.wallet_address)
                    .gas_limit(DEFAULT_GAS_LIMIT)
                    .into(),
            )
            .await;
        let result_value = result.value();

        println!("Result: {:?}", result_value);
    }

    async fn get_pool_address(&mut self) {
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result_value: ManagedAddress<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_pool_address(asset))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn pools_allowed(&mut self) {
        let result_value: MultiValueVec<ManagedAddress<DebugApi>> = self
            .interactor
            .vm_query(self.contract.pools_allowed())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn asset_loan_to_value(&mut self) {
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.asset_loan_to_value(asset))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn asset_liquidation_bonus(&mut self) {
        let asset = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.asset_liquidation_bonus(asset))
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

    async fn deposit_positions(&mut self) {
        let owner_nonce = 0u64;

        let result_value: MultiValueVec<MultiValue2<TokenIdentifier<DebugApi>, DepositPosition<DebugApi>>> = self
            .interactor
            .vm_query(self.contract.deposit_positions(owner_nonce))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn borrow_positions(&mut self) {
        let owner_nonce = 0u64;

        let result_value: MultiValueVec<MultiValue2<TokenIdentifier<DebugApi>, BorrowPosition<DebugApi>>> = self
            .interactor
            .vm_query(self.contract.borrow_positions(owner_nonce))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn nft_borrow_positions(&mut self) {
        let nft_nonce = 0u64;

        let result_value: BorrowPosition<DebugApi> = self
            .interactor
            .vm_query(self.contract.nft_borrow_positions(nft_nonce))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn collections(&mut self) {
        let result_value: MultiValueVec<TokenIdentifier<DebugApi>> = self
            .interactor
            .vm_query(self.contract.collections())
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn collection_params(&mut self) {
        let token = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result_value: CollectionParams<DebugApi> = self
            .interactor
            .vm_query(self.contract.collection_params(token))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_collateral_amount_for_token(&mut self) {
        let account_position = 0u64;
        let token_id = TokenIdentifier::from_esdt_bytes(&b""[..]);

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_collateral_amount_for_token(account_position, token_id))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_total_collateral_in_dollars(&mut self) {
        let account_position = 0u64;

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_total_collateral_in_dollars(account_position))
            .await;

        println!("Result: {:?}", result_value);
    }

    async fn get_total_borrow_in_dollars(&mut self) {
        let account_position = 0u64;

        let result_value: BigUint<DebugApi> = self
            .interactor
            .vm_query(self.contract.get_total_borrow_in_dollars(account_position))
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
