use common_events::MarketParams;
use common_proxies::proxy_onedex::State as StateOnedex;
use common_proxies::proxy_xexchange_pair::State as StateXExchange;
use common_structs::{
    AccountAttributes, AccountPosition, AccountPositionType, AssetConfig, EModeAssetConfig,
    EModeCategory, OracleProvider, PositionLimits,
};
use price_aggregator::structs::TimestampedPrice;
multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait Storage {
    /// Get the set of allowed pools
    /// This storage mapper holds the addresses of pools that are allowed to participate in the lending protocol.
    #[view(getPools)]
    #[storage_mapper("pools")]
    fn pools(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Get the account token
    /// This storage mapper holds the logic of the account token, which is a non-fungible token (NFT).
    #[view(getAccount)]
    #[storage_mapper("account")]
    fn account(&self) -> NonFungibleTokenMapper<Self::Api>;

    /// Get the account nonce
    /// This storage mapper holds the nonce of the account, which is a non-fungible token (NFT).
    #[view(getAccountNonce)]
    #[storage_mapper("account_nonce")]
    fn account_nonce(&self) -> SingleValueMapper<u64>;

    /// Get the account positions
    /// This storage mapper holds a list of account positions as a set. A position represents a nonce of an account (NFT nonce).
    #[view(getAccounts)]
    #[storage_mapper("accounts")]
    fn accounts(&self) -> UnorderedSetMapper<u64>;

    /// Get the account attributes
    /// This storage mapper maps each minted NFT to account attributes, useful for retrieving attributes without having the NFT in hand.
    #[view(getAccountAttributes)]
    #[storage_mapper("account_attributes")]
    fn account_attributes(&self, nonce: u64) -> SingleValueMapper<AccountAttributes<Self::Api>>;

    /// Get the deposit positions
    /// This storage mapper maps each deposit position to an account nonce, holding a list of assets and their corresponding structs.
    #[view(getPositions)]
    #[storage_mapper("positions")]
    fn positions(
        &self,
        nonce: u64,
        position_type: AccountPositionType,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    /// Get the liquidity pool template address
    /// This storage mapper holds the address of the liquidity pool template, used to create new liquidity pools.
    #[view(getLiqPoolTemplateAddress)]
    #[storage_mapper("liq_pool_template_address")]
    fn liq_pool_template_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the accumulator address
    /// This storage mapper holds the address of the accumulator, used to claim revenue from the liquidity pools.
    #[view(getAccumulatorAddress)]
    #[storage_mapper("accumulator_address")]
    fn accumulator_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the pools map
    /// This storage mapper holds a map of pools, used to get the address of a pool given a token ID.
    #[view(getPoolAddress)]
    #[storage_mapper("pools_map")]
    fn pools_map(&self, asset: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<ManagedAddress>;

    /// Get the price aggregator address
    /// This storage mapper holds the address of the price aggregator, used to get the price of a token in USD.
    #[view(getPriceAggregatorAddress)]
    #[storage_mapper("price_aggregator_address")]
    fn price_aggregator_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the safe price view address
    /// This storage mapper holds the address of the safe price view, used to get the price of a token out of the DEX pair.
    #[view(getSafePriceAddress)]
    #[storage_mapper("safe_price_view")]
    fn safe_price_view(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getSwapRouterAddress)]
    #[storage_mapper("swap_router_address")]
    fn swap_router(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the asset config
    /// This storage mapper holds the configuration of an asset, used to retrieve the config of an asset.
    #[view(getAssetConfig)]
    #[storage_mapper("asset_config")]
    fn asset_config(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<AssetConfig<Self::Api>>;

    /// Get the last e-mode category ID
    /// This storage mapper holds the ID of the last e-mode category, used to retrieve the last e-mode category.
    #[view(lastEModeCategoryId)]
    #[storage_mapper("last_e_mode_category_id")]
    fn last_e_mode_category_id(&self) -> SingleValueMapper<u8>;

    /// Get all e-mode categories
    /// This storage mapper holds a map of e-mode categories, used to group assets into categories with different risk parameters.
    #[view(getEModes)]
    #[storage_mapper("e_mode_category")]
    fn e_mode_categories(&self) -> MapMapper<u8, EModeCategory<Self::Api>>;

    /// Get the e-mode categories for a given asset
    /// This storage mapper holds a set of e-mode categories for a given asset. One asset can have multiple e-mode categories.
    #[view(getAssetEModes)]
    #[storage_mapper("asset_e_modes")]
    fn asset_e_modes(&self, asset: &EgldOrEsdtTokenIdentifier) -> UnorderedSetMapper<u8>;

    /// Get all assets for a given e-mode category
    /// This storage mapper holds a map of assets for a given e-mode category, used to get the config for a given asset in a given e-mode category.
    #[view(getEModesAssets)]
    #[storage_mapper("e_mode_assets")]
    fn e_mode_assets(&self, id: u8) -> MapMapper<EgldOrEsdtTokenIdentifier, EModeAssetConfig>;

    /// Get the debt in USD for isolated assets
    /// This storage mapper holds the debt in USD for isolated assets.
    #[view(getIsolatedAssetDebtUsd)]
    #[storage_mapper("isolated_asset_debt_usd")]
    fn isolated_asset_debt_usd(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>>;

    /// Get the token oracle
    /// This storage mapper holds the oracle of a token, used to get the price of a token.
    #[view(getTokenOracle)]
    #[storage_mapper("token_oracle")]
    fn token_oracle(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<OracleProvider<Self::Api>>;

    // Reentrancy guard
    #[view(isFlashLoanOngoing)]
    #[storage_mapper("flash_loan_ongoing")]
    fn flash_loan_ongoing(&self) -> SingleValueMapper<bool>;

    /// Get the position limits configuration
    /// This storage mapper holds the maximum number of borrow and supply positions per NFT
    /// Used to optimize gas costs during liquidations and prevent excessive position complexity
    #[view(getPositionLimits)]
    #[storage_mapper("position_limits")]
    fn position_limits(&self) -> SingleValueMapper<PositionLimits>;

    /// PROXY STORAGE ///
    ///
    /// Retrieves the total scaled amount supplied to the pool.
    /// This value represents the sum of all supplied principals, each divided by the supply index at the time of their deposit.
    /// It is stored RAY-scaled.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total scaled amount supplied, RAY-scaled.
    #[storage_mapper_from_address("supplied")]
    fn supplied(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress>;

    /// Retrieves the total scaled borrowed amount from the pool.
    /// This value represents the sum of all borrowed principals, each divided by the borrow index at the time of their borrowing.
    /// It is stored RAY-scaled.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The total scaled borrowed amount, RAY-scaled.
    #[storage_mapper_from_address("borrowed")]
    fn borrowed(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress>;

    /// Retrieves the current borrow index.
    ///
    /// The borrow index is used to calculate accrued interest on borrow positions.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current borrow index.
    #[storage_mapper_from_address("borrow_index")]
    fn borrow_index(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress>;

    /// Retrieves the current supply index.
    ///
    /// The supply index is used to compute the yield for suppliers.
    ///
    /// # Returns
    /// - `ManagedDecimal<Self::Api, NumDecimals>`: The current supply index.
    #[storage_mapper_from_address("supply_index")]
    fn supply_index(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<ManagedDecimal<Self::Api, NumDecimals>, ManagedAddress>;

    /// Returns the market parameters.
    ///
    /// These include interest rate parameters and asset decimals.
    ///
    /// # Returns
    /// - `MarketParams<Self::Api>`: The market configuration.
    #[storage_mapper_from_address("params")]
    fn parameters(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<MarketParams<Self::Api>, ManagedAddress>;

    /// Retrieves the last update timestamp for the interest indexes.
    ///
    /// # Returns
    /// - `u64`: The timestamp when indexes were last updated.
    #[storage_mapper_from_address("last_timestamp")]
    fn last_timestamp(
        &self,
        liquidity_pool_address: ManagedAddress,
    ) -> SingleValueMapper<u64, ManagedAddress>;

    #[storage_mapper_from_address("rounds")]
    fn rounds(
        &self,
        price_aggregator_address: ManagedAddress,
        from: ManagedBuffer,
        to: ManagedBuffer,
    ) -> SingleValueMapper<TimestampedPrice<Self::Api>, ManagedAddress>;

    #[storage_mapper_from_address("state")]
    fn xexchange_pair_state(
        &self,
        dex_address: ManagedAddress,
    ) -> SingleValueMapper<StateXExchange, ManagedAddress>;

    #[storage_mapper_from_address("pair_state")]
    fn onedex_pair_state(
        &self,
        dex_address: ManagedAddress,
        pair_id: usize,
    ) -> SingleValueMapper<StateOnedex, ManagedAddress>;

    #[storage_mapper_from_address("pause_module:paused")]
    fn price_aggregator_paused_state(
        &self,
        price_aggregator_address: ManagedAddress,
    ) -> SingleValueMapper<bool, ManagedAddress>;
}
