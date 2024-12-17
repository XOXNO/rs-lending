use common_events::{AssetConfig, EModeAssetConfig, EModeCategory, OracleProvider};
use common_structs::{AccountPosition, NftAccountAttributes};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait LendingStorageModule {
    /// Get the account token
    /// The storage holds the logic of the account token
    #[view(getAccountToken)]
    #[storage_mapper("account_token")]
    fn account_token(&self) -> NonFungibleTokenMapper<Self::Api>;

    /// Get the account positions
    /// The storage holds a list of account positions as a set
    /// A position represents a nonce of an account (NFT nonce)
    #[view(getAccountPositions)]
    #[storage_mapper("account_positions")]
    fn account_positions(&self) -> UnorderedSetMapper<u64>;

    /// Get the account attributes
    /// We are mapping each minted NFT to an account attributes
    /// Useful when we want to know the attributes of an account without having the NFT in hand
    #[view(getAccountAttributes)]
    #[storage_mapper("account_attributes")]
    fn account_attributes(&self, nonce: u64) -> SingleValueMapper<NftAccountAttributes>;

    /// Get the deposit positions
    /// We are mapping each deposit position to an account nonce
    /// A deposit position is a list of assets and their corresponding structs
    #[view(getDepositPositions)]
    #[storage_mapper("deposit_positions")]
    fn deposit_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    /// Get the borrow positions
    /// We are mapping each borrow position to an account nonce
    /// A borrow position is a list of assets and their corresponding structs
    #[view(getBorrowPositions)]
    #[storage_mapper("borrow_positions")]
    fn borrow_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    /// Get the liq pool template address
    /// The storage holds the address of the liq pool template
    /// The liq pool template is used to create new liquidity pools
    #[view(getLiqPoolTemplateAddress)]
    #[storage_mapper("liq_pool_template_address")]
    fn liq_pool_template_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the accumulator address
    /// The storage holds the address of the accumulator
    /// The accumulator is used to claim the revenue from the liquidity pools
    #[view(getAccumulatorAddress)]
    #[storage_mapper("accumulator_address")]
    fn accumulator_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the pools map
    /// The storage holds a map of pools
    /// The map is used to get the address of a pool given a token id
    #[view(getPoolsMap)]
    #[storage_mapper("pools_map")]
    fn pools_map(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<ManagedAddress>;

    /// Get the price aggregator address
    /// The storage holds the address of the price aggregator
    /// The price aggregator is used to get the price of a token in USD
    #[view(getPriceAggregatorAddress)]
    #[storage_mapper("price_aggregator_address")]
    fn price_aggregator_address(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the safe price view address
    /// The storage holds the address of the safe price view
    /// The safe price view is used to get the price of a token out of the DEX pair
    #[view(getSafePriceView)]
    #[storage_mapper("safe_price_view")]
    fn safe_price_view(&self) -> SingleValueMapper<ManagedAddress>;

    /// Get the asset config
    /// The storage holds the config of an asset
    /// The config is used to get the config of an asset
    #[view(getAssetConfig)]
    #[storage_mapper("asset_config")]
    fn asset_config(
        &self,
        asset: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<AssetConfig<Self::Api>>;

    /// Get the asset LTV
    /// The storage holds the LTV of an asset
    /// The LTV is used to get the LTV of an asset
    #[view(getAssetLTV)]
    #[storage_mapper("asset_ltv")]
    fn asset_ltv(&self, asset: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;

    /// Get the last e-mode category id
    /// The storage holds the id of the last e-mode category
    /// The id is used to get the last e-mode category
    #[view(lastEModeCategoryId)]
    #[storage_mapper("last_e_mode_category_id")]
    fn last_e_mode_category_id(&self) -> SingleValueMapper<u8>;

    // Get all e-mode categories
    // E-mode categories are used to group assets into categories with different risk parameters
    #[view(getEModes)]
    #[storage_mapper("e_mode_category")]
    fn e_mode_category(&self) -> MapMapper<u8, EModeCategory<Self::Api>>;

    // Get the e-mode categories for a given asset
    // One asset can have multiple e-mode categories
    #[view(getAssetEModes)]
    #[storage_mapper("asset_e_modes")]
    fn asset_e_modes(&self, asset: &EgldOrEsdtTokenIdentifier) -> UnorderedSetMapper<u8>;

    // Get all assets for a given e-mode category
    // Get the config for a given asset in a given e-mode category such as can be used as collateral or can be borrowed
    #[view(getEModesAssets)]
    #[storage_mapper("e_mode_assets")]
    fn e_mode_assets(&self, id: u8) -> MapMapper<EgldOrEsdtTokenIdentifier, EModeAssetConfig>;

    // Debt in USD for isolated assets
    #[view(getIsolatedAssetDebtUsd)]
    #[storage_mapper("isolated_asset_debt_usd")]
    fn isolated_asset_debt_usd(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    // Vault supplied amount per token
    #[view(getVaultSuppliedAmount)]
    #[storage_mapper("vault_supplied_amount")]
    fn vault_supplied_amount(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<BigUint>;

    /// Get the token oracle
    /// The storage holds the oracle of a token
    /// The oracle is used to get the price of a token
    #[view(getTokenOracle)]
    #[storage_mapper("token_oracle")]
    fn token_oracle(
        &self,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> SingleValueMapper<OracleProvider<Self::Api>>;

    /// Get the last token price in EGLD
    /// The price is used to get the price of a token in EGLD in case of a price oracle failure or big deviation
    #[view(getLastTokenPrice)]
    #[storage_mapper("last_token_price")]
    fn last_token_price(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;
}
