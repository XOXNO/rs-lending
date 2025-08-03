use common_structs::{AssetConfig, EModeAssetConfig, EModeCategory};

use crate::storage;
use common_errors::{
    ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS, ERROR_EMODE_CATEGORY_DEPRECATED,
    ERROR_EMODE_CATEGORY_NOT_FOUND,
};

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait EModeModule: storage::Storage {
    /// Applies e-mode configuration to an asset if applicable.
    ///
    /// **Purpose**: Enhances asset risk parameters when e-mode is active for correlated
    /// assets, providing more favorable borrowing conditions while maintaining safety.
    ///
    /// **Methodology**:
    /// 1. Validates both e-mode category and asset config are present
    /// 2. Updates asset borrowing and collateral flags from e-mode config
    /// 3. Applies enhanced risk parameters from e-mode category
    /// 4. Leaves parameters unchanged if e-mode not applicable
    ///
    /// **Enhanced Parameters**:
    /// - `loan_to_value`: Higher LTV for correlated assets
    /// - `liquidation_threshold`: Optimized threshold for asset correlation
    /// - `liquidation_bonus`: Adjusted bonus for e-mode liquidations
    ///
    /// **Security Considerations**:
    /// - Only applies to explicitly configured asset-emode pairs
    /// - Enhanced parameters validated during e-mode setup
    /// - Asset correlation assumptions built into category design
    ///
    /// **Risk Management**:
    /// - E-mode categories designed for correlated asset groups
    /// - Parameters tuned for reduced volatility within category
    /// - Maintains protocol safety through correlation analysis
    ///
    /// # Arguments
    /// - `asset_config`: Mutable asset configuration to enhance
    /// - `category`: Optional e-mode category with enhanced parameters
    /// - `asset_emode_config`: Optional asset-specific e-mode configuration
    fn apply_e_mode_to_asset_config(
        &self,
        asset_config: &mut AssetConfig<Self::Api>,
        category: &Option<EModeCategory<Self::Api>>,
        asset_emode_config: Option<EModeAssetConfig>,
    ) {
        if let (Some(category), Some(asset_emode_config)) = (category, asset_emode_config) {
            asset_config.is_collateralizable = asset_emode_config.is_collateralizable;
            asset_config.is_borrowable = asset_emode_config.is_borrowable;
            asset_config.loan_to_value_bps = category.loan_to_value_bps.clone();
            asset_config.liquidation_threshold_bps = category.liquidation_threshold_bps.clone();
            asset_config.liquidation_bonus_bps = category.liquidation_bonus_bps.clone();
        }
    }

    /// Ensures an e-mode category is not deprecated.
    ///
    /// **Purpose**: Prevents usage of deprecated e-mode categories that may have
    /// outdated risk parameters or security vulnerabilities.
    ///
    /// **Methodology**:
    /// - Checks deprecation flag if category is present
    /// - Allows operations to proceed if no e-mode active
    /// - Blocks operations with deprecated categories
    ///
    /// **Deprecation Scenarios**:
    /// - Risk parameters no longer suitable for market conditions
    /// - Asset correlations changed, invalidating category assumptions
    /// - Security issues discovered in category configuration
    /// - Governance decisions to phase out categories
    ///
    /// **Security Rationale**:
    /// - Prevents usage of potentially unsafe risk parameters
    /// - Protects users from outdated correlation assumptions
    /// - Enables graceful migration to updated categories
    ///
    /// # Arguments
    /// - `category`: Optional e-mode category to validate for deprecation
    fn ensure_e_mode_not_deprecated(&self, category: &Option<EModeCategory<Self::Api>>) {
        match category {
            Some(cat) => require!(!cat.is_deprecated(), ERROR_EMODE_CATEGORY_DEPRECATED),
            None => {
                // No category, do nothing
            },
        }
    }

    /// Ensures e-mode compatibility with isolated assets.
    ///
    /// **Purpose**: Enforces mutual exclusion between e-mode and isolation mode
    /// to prevent conflicting risk management strategies.
    ///
    /// **Methodology**:
    /// - Checks if asset is marked as isolated
    /// - Validates e-mode is disabled (ID = 0) for isolated assets
    /// - Allows e-mode for non-isolated assets
    ///
    /// **Risk Management Rationale**:
    /// - Isolated assets have specific concentration limits
    /// - E-mode assumes asset correlation, incompatible with isolation
    /// - Mixing strategies could lead to unexpected risk exposure
    /// - Debt ceilings in isolation mode conflict with e-mode assumptions
    ///
    /// **Policy Enforcement**:
    /// - Isolated assets must use standard risk parameters
    /// - E-mode assets cannot be used in isolation mode
    /// - Clear separation of risk management strategies
    ///
    /// # Arguments
    /// - `asset_config`: Asset configuration with isolation flag
    /// - `e_mode_id`: E-mode category ID (0 = disabled)
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
    /// **Purpose**: Safely loads asset-specific e-mode configuration with validation
    /// to ensure both category and asset are properly configured for e-mode.
    ///
    /// **Methodology**:
    /// 1. Returns None if e-mode is disabled (ID = 0)
    /// 2. Validates asset is registered for the e-mode category
    /// 3. Validates category includes the specific asset
    /// 4. Returns asset-specific e-mode configuration
    ///
    /// **Validation Steps**:
    /// - Asset must be in category's asset list
    /// - Category must be in asset's e-mode list
    /// - Bidirectional validation prevents configuration mismatches
    ///
    /// **Security Considerations**:
    /// - Prevents unauthorized e-mode application
    /// - Ensures configuration consistency across mappings
    /// - Validates both directions of the relationship
    ///
    /// **Configuration Structure**:
    /// ```
    /// asset_e_modes[token_id] -> [category_ids]
    /// e_mode_assets[category_id][token_id] -> asset_config
    /// ```
    ///
    /// # Arguments
    /// - `e_mode_id`: E-mode category ID to validate
    /// - `token_id`: Token identifier for configuration lookup
    ///
    /// # Returns
    /// - `Option<EModeAssetConfig>` with asset-specific e-mode settings
    fn token_e_mode_config(
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

        e_mode_assets.get(token_id)
    }

    /// Retrieves a valid e-mode category.
    ///
    /// **Purpose**: Safely loads e-mode category configuration with existence
    /// validation to ensure category is properly configured and active.
    ///
    /// **Methodology**:
    /// 1. Returns None if e-mode is disabled (ID = 0)
    /// 2. Validates category exists in storage
    /// 3. Returns complete category configuration
    ///
    /// **Category Configuration**:
    /// - Enhanced risk parameters for correlated assets
    /// - Borrowing and collateral eligibility rules
    /// - Deprecation status for lifecycle management
    ///
    /// **Security Considerations**:
    /// - Validates category existence before use
    /// - Prevents operations with invalid category IDs
    /// - Ensures configuration completeness
    ///
    /// **Use Cases**:
    /// - Risk parameter enhancement for positions
    /// - Category validation during operations
    /// - Deprecation status checking
    ///
    /// # Arguments
    /// - `e_mode_id`: E-mode category ID to retrieve
    ///
    /// # Returns
    /// - `Option<EModeCategory>` with complete category configuration
    fn e_mode_category(&self, e_mode_id: u8) -> Option<EModeCategory<Self::Api>> {
        if e_mode_id == 0 {
            return None;
        }

        let e_mode_categories = self.e_mode_categories();
        require!(
            e_mode_categories.contains_key(&e_mode_id),
            ERROR_EMODE_CATEGORY_NOT_FOUND
        );

        e_mode_categories.get(&e_mode_id)
    }
}
