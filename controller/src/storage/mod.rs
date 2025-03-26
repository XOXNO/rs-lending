use common_structs::{
    AccountAttributes, AccountPosition, AssetConfig, EModeAssetConfig, EModeCategory,
    OracleProvider,
};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait Storage {
    /// Get the set of allowed pools
    /// This storage mapper holds the addresses of pools that are allowed to participate in the lending protocol.
    #[view(getPoolAllowed)]
    #[storage_mapper("pool_allowed")]
    fn pools_allowed(&self) -> UnorderedSetMapper<ManagedAddress>;

    /// Get the account token
    /// This storage mapper holds the logic of the account token, which is a non-fungible token (NFT).
    #[view(getAccountToken)]
    #[storage_mapper("account_token")]
    fn account_token(&self) -> NonFungibleTokenMapper<Self::Api>;

    /// Get the account positions
    /// This storage mapper holds a list of account positions as a set. A position represents a nonce of an account (NFT nonce).
    #[view(getAccountPositions)]
    #[storage_mapper("account_positions")]
    fn account_positions(&self) -> UnorderedSetMapper<u64>;

    /// Get the account attributes
    /// This storage mapper maps each minted NFT to account attributes, useful for retrieving attributes without having the NFT in hand.
    #[view(getAccountAttributes)]
    #[storage_mapper("account_attributes")]
    fn account_attributes(&self, nonce: u64) -> SingleValueMapper<AccountAttributes<Self::Api>>;

    /// Get the deposit positions
    /// This storage mapper maps each deposit position to an account nonce, holding a list of assets and their corresponding structs.
    #[view(getDepositPositions)]
    #[storage_mapper("deposit_positions")]
    fn deposit_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    /// Get the borrow positions
    /// This storage mapper maps each borrow position to an account nonce, holding a list of assets and their corresponding structs.
    #[view(getBorrowPositions)]
    #[storage_mapper("borrow_positions")]
    fn borrow_positions(
        &self,
        owner_nonce: u64,
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

    /// This storage mapper holds the address of the wrapper, used to convert between EGLD <-> WEGLD
    #[view(getEGLDWrapperAddress)]
    #[storage_mapper("wegld_wrapper_address")]
    fn wegld_wrapper(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getAggregatorAddress)]
    #[storage_mapper("aggregator_address")]
    fn aggregator(&self) -> SingleValueMapper<ManagedAddress>;

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
    fn e_mode_category(&self) -> MapMapper<u8, EModeCategory<Self::Api>>;

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

    /// Get the vault supplied amount per token
    /// This storage mapper holds the supplied amount per token in the vault.
    #[view(getVaultSuppliedAmount)]
    #[storage_mapper("vault_supplied_amount")]
    fn vault_supplied_amount(
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
}
