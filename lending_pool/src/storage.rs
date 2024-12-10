use common_events::{AssetConfig, EModeAssetConfig, EModeCategory, OracleProvider};
use common_structs::AccountPosition;

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait LendingStorageModule {
    #[view(getDepositPositions)]
    #[storage_mapper("deposit_positions")]
    fn deposit_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    #[view(getBorrowPositions)]
    #[storage_mapper("borrow_positions")]
    fn borrow_positions(
        &self,
        owner_nonce: u64,
    ) -> MapMapper<EgldOrEsdtTokenIdentifier, AccountPosition<Self::Api>>;

    #[view(getPoolsMap)]
    #[storage_mapper("pools_map")]
    fn pools_map(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<ManagedAddress>;

    #[view(getPoolAllowed)]
    #[storage_mapper("pool_allowed")]
    fn pools_allowed(&self) -> UnorderedSetMapper<ManagedAddress>;

    #[view(getPriceAggregatorAddress)]
    #[storage_mapper("price_aggregator_address")]
    fn price_aggregator_address(&self) -> SingleValueMapper<ManagedAddress>;

    #[view(getSafePriceView)]
    #[storage_mapper("safe_price_view")]
    fn safe_price_view(&self) -> SingleValueMapper<ManagedAddress>;

    ///////
    // Asset config
    ///////
    // Get the config for a given asset
    // Contains the latest parameters for a given asset
    // Used to check if an asset is supported, what the liquidation threshold is, etc.
    #[view(getAssetConfig)]
    #[storage_mapper("asset_config")]
    fn asset_config(&self, asset: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<AssetConfig<Self::Api>>;

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
    fn isolated_asset_debt_usd(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;

    // Vault supplied amount per token
    #[view(getVaultSuppliedAmount)]
    #[storage_mapper("vault_supplied_amount")]
    fn vault_supplied_amount(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<BigUint>;

    #[view(getTokenOracle)]
    #[storage_mapper("token_oracle")] // LXOXNO, LPEGLD, XEGLD (market token)
    fn token_oracle(&self, token_id: &EgldOrEsdtTokenIdentifier) -> SingleValueMapper<OracleProvider<Self::Api>>;
}
