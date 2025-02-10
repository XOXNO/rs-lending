use common_events::{AssetConfig, EModeAssetConfig, EModeCategory};

use crate::{
    helpers, oracle, storage, utils, validation, ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS,
    ERROR_EMODE_CATEGORY_DEPRECATED, ERROR_EMODE_CATEGORY_NOT_FOUND,
};

use super::{account, borrow, repay, update, withdraw};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait EModeModule: storage::LendingStorageModule {
    /// Updates asset configuration for e-mode
    ///
    /// # Arguments
    /// * `asset_info` - Asset configuration to update
    /// * `e_mode_category_id` - E-mode category ID
    /// * `token_id` - Token identifier
    ///
    /// If position is in e-mode and asset supports it,
    /// updates LTV, liquidation threshold, and other parameters
    /// based on e-mode category settings.
    fn update_asset_config_for_e_mode(
        &self,
        asset_info: &mut AssetConfig<Self::Api>,
        category: &Option<EModeCategory<Self::Api>>,
        asset_emode_config: Option<EModeAssetConfig>,
    ) {
        if let (Some(category), Some(asset_emode_config)) = (category, asset_emode_config) {
            // Update all asset config parameters with e-mode values for that category
            asset_info.can_be_collateral = asset_emode_config.can_be_collateral;
            asset_info.can_be_borrowed = asset_emode_config.can_be_borrowed;
            asset_info.ltv = category.ltv.clone();
            asset_info.liquidation_threshold = category.liquidation_threshold.clone();
            asset_info.liquidation_base_bonus = category.liquidation_bonus.clone();
        }
    }

    fn validate_not_depracated_e_mode(&self, e_mode_category: &Option<EModeCategory<Self::Api>>) {
        if let Some(category) = e_mode_category {
            require!(!category.is_deprecated, ERROR_EMODE_CATEGORY_DEPRECATED);
        }
    }

    fn validate_e_mode_not_isolated(&self, asset_info: &AssetConfig<Self::Api>, e_mode: u8) {
        require!(
            !(asset_info.is_isolated && e_mode != 0),
            ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS
        );
    }

    fn validate_token_of_emode(
        &self,
        e_mode: u8,
        token_id: &EgldOrEsdtTokenIdentifier,
    ) -> Option<EModeAssetConfig> {
        if e_mode == 0 {
            return None;
        }

        require!(
            self.asset_e_modes(token_id).contains(&e_mode),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        let e_mode_mapper = self.e_mode_assets(e_mode);
        // Validate asset has configuration for this e-mode
        require!(
            e_mode_mapper.contains_key(token_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        Some(e_mode_mapper.get(token_id).unwrap())
    }

    fn validate_e_mode_exists(&self, e_mode: u8) -> Option<EModeCategory<Self::Api>> {
        if e_mode == 0 {
            return None;
        }
        let e_mode_mapper = self.e_mode_category();
        require!(
            e_mode_mapper.contains_key(&e_mode),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );
        Some(e_mode_mapper.get(&e_mode).unwrap())
    }
}
