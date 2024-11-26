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
        "getPoolAsset" => interact.pool_asset().await,
        "getReserves" => interact.reserves().await,
        "getSuppliedAmount" => interact.supplied_amount().await,
        "getRewardsReserves" => interact.rewards_reserves().await,
        "getTotalBorrow" => interact.borrowed_amount().await,
        "getPoolParams" => interact.pool_params().await,
        "getBorrowIndex" => interact.borrow_index().await,
        "getSupplyIndex" => interact.supply_index().await,
        "borrowIndexLastUpdateRound" => interact.borrow_index_last_update_round().await,
        "getAccountToken" => interact.account_token().await,
        "getAccountPositions" => interact.account_positions().await,
        "updatePositionInterest" => interact.update_collateral_with_interest().await,
        "updatePositionDebt" => interact.update_borrows_with_debt().await,
        "supply" => interact.supply().await,
        "borrow" => interact.borrow().await,
        "withdraw" => interact.withdraw().await,
        "repay" => interact.repay().await,
        "flashLoan" => interact.flash_loan().await,
        "getCapitalUtilisation" => interact.get_capital_utilisation().await,
        "getTotalCapital" => interact.get_total_capital().await,
        "getDebtInterest" => interact.get_debt_interest().await,
        "getDepositRate" => interact.get_deposit_rate().await,
        "getBorrowRate" => interact.get_borrow_rate().await,
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
        interactor.set_current_dir_from_workspace("liquidity_pool");

        let wallet_address = interactor.register_wallet(test_wallets::alice()).await;
        
        let contract_code = BytesValue::interpret_from(
            "mxsc:../output/liquidity_pool.mxsc.json",
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
        let asset = EgldOrEsdtTokenIdentifier::esdt(&b""[..]);
        let r_max = BigUint::<StaticApi>::from(0u128);
        let r_base = BigUint::<StaticApi>::from(0u128);
        let r_slope1 = BigUint::<StaticApi>::from(0u128);
        let r_slope2 = BigUint::<StaticApi>::from(0u128);
        let u_optimal = BigUint::<StaticApi>::from(0u128);
        let reserve_factor = BigUint::<StaticApi>::from(0u128);

        let new_address = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .init(asset, r_max, r_base, r_slope1, r_slope2, u_optimal, reserve_factor)
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
        let r_max = BigUint::<StaticApi>::from(0u128);
        let r_base = BigUint::<StaticApi>::from(0u128);
        let r_slope1 = BigUint::<StaticApi>::from(0u128);
        let r_slope2 = BigUint::<StaticApi>::from(0u128);
        let u_optimal = BigUint::<StaticApi>::from(0u128);
        let reserve_factor = BigUint::<StaticApi>::from(0u128);

        let response = self
            .interactor
            .tx()
            .to(self.state.current_address())
            .from(&self.wallet_address)
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .upgrade(r_max, r_base, r_slope1, r_slope2, u_optimal, reserve_factor)
            .code(&self.contract_code)
            .code_metadata(CodeMetadata::UPGRADEABLE)
            .returns(ReturnsNewAddress)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn pool_asset(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .pool_asset()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn reserves(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .reserves()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn supplied_amount(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .supplied_amount()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn rewards_reserves(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .rewards_reserves()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn borrowed_amount(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .borrowed_amount()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn pool_params(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .pool_params()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn borrow_index(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .borrow_index()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn supply_index(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .supply_index()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn borrow_index_last_update_round(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .borrow_index_last_update_round()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn account_token(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .account_token()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn account_positions(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .account_positions()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn update_collateral_with_interest(&mut self) {
        let deposit_position = AccountPosition::<StaticApi>::default();

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .update_collateral_with_interest(deposit_position)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn update_borrows_with_debt(&mut self) {
        let borrow_position = AccountPosition::<StaticApi>::default();

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .update_borrows_with_debt(borrow_position)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn supply(&mut self) {
        let token_id = String::new();
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(0u128);

        let deposit_position = AccountPosition::<StaticApi>::default();

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .supply(deposit_position)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn borrow(&mut self) {
        let token_id = String::new();
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_amount = BigUint::<StaticApi>::from(0u128);
        let existing_borrow_position = AccountPosition::<StaticApi>::default();

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .borrow(initial_caller, borrow_amount, existing_borrow_position)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn withdraw(&mut self) {
        let token_id = String::new();
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let amount = BigUint::<StaticApi>::from(0u128);
        let deposit_position = AccountPosition::<StaticApi>::default();
        let is_liquidation = bool::<StaticApi>::default();
        let protocol_liquidation_fee = BigUint::<StaticApi>::from(0u128);

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .withdraw(initial_caller, amount, deposit_position, is_liquidation, protocol_liquidation_fee)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn repay(&mut self) {
        let token_id = String::new();
        let token_nonce = 0u64;
        let token_amount = BigUint::<StaticApi>::from(0u128);

        let initial_caller = bech32::decode("");
        let borrow_position = AccountPosition::<StaticApi>::default();

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .repay(initial_caller, borrow_position)
            .payment((TokenIdentifier::from(token_id.as_str()), token_nonce, token_amount))
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn flash_loan(&mut self) {
        let borrowed_token = EgldOrEsdtTokenIdentifier::esdt(&b""[..]);
        let amount = BigUint::<StaticApi>::from(0u128);
        let contract_address = bech32::decode("");
        let endpoint = ManagedBuffer::new_from_bytes(&b""[..]);
        let arguments = ManagedVec::from_single_item(ManagedBuffer::new_from_bytes(&b""[..]));
        let fees = BigUint::<StaticApi>::from(0u128);

        let response = self
            .interactor
            .tx()
            .from(&self.wallet_address)
            .to(self.state.current_address())
            .gas(30_000_000u64)
            .typed(proxy::LiquidityPoolProxy)
            .flash_loan(borrowed_token, amount, contract_address, endpoint, arguments, fees)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {response:?}");
    }

    async fn get_capital_utilisation(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .get_capital_utilisation()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn get_total_capital(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .get_total_capital()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn get_debt_interest(&mut self) {
        let amount = BigUint::<StaticApi>::from(0u128);
        let initial_borrow_index = BigUint::<StaticApi>::from(0u128);

        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .get_debt_interest(amount, initial_borrow_index)
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn get_deposit_rate(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .get_deposit_rate()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

    async fn get_borrow_rate(&mut self) {
        let result_value = self
            .interactor
            .query()
            .to(self.state.current_address())
            .typed(proxy::LiquidityPoolProxy)
            .get_borrow_rate()
            .returns(ReturnsResultUnmanaged)
            
            .run()
            .await;

        println!("Result: {result_value:?}");
    }

}
