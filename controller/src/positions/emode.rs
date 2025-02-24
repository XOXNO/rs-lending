use common_structs::{AssetConfig, EModeAssetConfig, EModeCategory};

use crate::storage;
use common_errors::{
    ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS, ERROR_EMODE_CATEGORY_DEPRECATED,
    ERROR_EMODE_CATEGORY_NOT_FOUND,
};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait EModeModule: storage::LendingStorageModule {
    /// Applies e-mode configuration to an asset if applicable.
    /// Updates risk parameters based on e-mode settings.
    ///
    /// # Arguments
    /// - `asset_config`: Mutable asset configuration.
    /// - `category`: Optional e-mode category.
    /// - `asset_emode_config`: Optional e-mode config for the asset.
    fn apply_e_mode_to_asset_config(
        &self,
        asset_config: &mut AssetConfig<Self::Api>,
        category: &Option<EModeCategory<Self::Api>>,
        asset_emode_config: Option<EModeAssetConfig>,
    ) {
        if let (Some(category), Some(asset_emode_config)) = (category, asset_emode_config) {
            asset_config.is_collateralizable = asset_emode_config.is_collateralizable;
            asset_config.is_borrowable = asset_emode_config.is_borrowable;
            asset_config.loan_to_value = category.loan_to_value.clone();
            asset_config.liquidation_threshold = category.liquidation_threshold.clone();
            asset_config.liquidation_bonus = category.liquidation_bonus.clone();
        }
    }

    /// Ensures an e-mode category is not deprecated.
    ///
    /// # Arguments
    /// - `category`: Optional e-mode category to check.
    fn ensure_e_mode_not_deprecated(&self, category: &Option<EModeCategory<Self::Api>>) {
        if let Some(cat) = category {
            require!(!cat.is_deprecated(), ERROR_EMODE_CATEGORY_DEPRECATED);
        }
    }

    /// Ensures e-mode compatibility with isolated assets.
    /// Prevents e-mode use with isolated assets unless disabled.
    ///
    /// # Arguments
    /// - `asset_config`: Asset configuration.
    /// - `e_mode_id`: E-mode category ID.
    fn ensure_e_mode_compatible_with_asset(
        &self,
        asset_config: &AssetConfig<Self::Api>,
        e_mode_id: u8,
    ) {
        require!(
            !(asset_config.is_isolated() && e_mode_id != 0),
            ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
        );
    }

    /// Retrieves valid e-mode configuration for a token.
    ///
    /// # Arguments
    /// - `e_mode_id`: E-mode category ID.
    /// - `token_id`: Token identifier.
    ///
    /// # Returns
    /// - Optional `EModeAssetConfig` if valid.
    fn get_token_e_mode_config(
        &self,
        e_mode_id: u8,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> Option<EModeAssetConfig> {
        if e_mode_id == 0 {
            return None;
        }
        let asset_e_modes = self.asset_e_modes(token_id);
        require!(
            asset_e_modes.contains(&e_mode_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        let e_mode_assets = self.e_mode_assets(e_mode_id);
        require!(
            e_mode_assets.contains_key(token_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        Some(e_mode_assets.get(token_id).unwrap())
    }

    /// Retrieves a valid e-mode category.
    ///
    /// # Arguments
    /// - `e_mode_id`: E-mode category ID.
    ///
    /// # Returns
    /// - Optional `EModeCategory` if valid.
    fn get_e_mode_category(&self, e_mode_id: u8) -> Option<EModeCategory<Self::Api>> {
        if e_mode_id == 0 {
            return None;
        }
        let e_mode_categories = self.e_mode_category();
        require!(
            e_mode_categories.contains_key(&e_mode_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        Some(e_mode_categories.get(&e_mode_id).unwrap())
    }
}
