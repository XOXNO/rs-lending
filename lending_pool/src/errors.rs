pub static ERROR_ASSET_NOT_SUPPORTED: &[u8] = b"Asset not supported.";
pub static ERROR_INSUFFICIENT_COLLATERAL: &[u8] = b"Not enough collateral available for this loan.";
pub static ERROR_INSUFFICIENT_DEPOSIT: &[u8] = b"Not enough tokens deposited for this account.";
pub static ERROR_HEALTH_FACTOR: &[u8] = b"Health not low enough for liquidation.";
pub static ERROR_HEALTH_FACTOR_WITHDRAW: &[u8] = b"Health factor will be too low after withdrawal.";
pub static ERROR_TOKEN_MISMATCH: &[u8] = b"Token sent is not the same as the liquidation token.";
pub static ERROR_INSUFFICIENT_LIQUIDATION: &[u8] = b"Insufficient funds for liquidation.";
pub static ERROR_NO_COLLATERAL_TOKEN: &[u8] =
    b"Liquidatee user doesn't have this token as collateral.";

pub static ERROR_ASSET_ALREADY_SUPPORTED: &[u8] = b"Asset already supported.";
pub static ERROR_INVALID_TICKER: &[u8] = b"Invalid ticker provided.";
pub static ERROR_NO_POOL_FOUND: &[u8] = b"No pool found for this asset.";

pub static ERROR_TEMPLATE_EMPTY: &[u8] = b"Liquidity pool contract template is empty.";

pub static ERROR_TOKEN_TICKER_FETCH: &[u8] = b"Failed to get token ticker.";

pub static ERROR_INSUFFICIENT_LIQUIDITY: &[u8] = b"Insufficient liquidity in pool";

pub static ERROR_COLLATERAL_NOT_FOUND: &[u8] = b"Collateral not found to liquidate.";

pub static ERROR_PRICE_AGGREGATOR_NOT_SET: &[u8] = b"Price aggregator not set.";
pub static ERROR_INVALID_NUMBER_OF_ESDT_TRANSFERS: &[u8] = b"Invalid number of ESDT transfers";

pub static ERROR_INVALID_LIQUIDATION_THRESHOLD: &[u8] =
    b"Invalid liquidation threshold has to be higher than the loan-to-value.";

pub static ERROR_EMODE_CATEGORY_NOT_FOUND: &[u8] = b"E-mode category not found.";

pub static ERROR_ASSET_ALREADY_SUPPORTED_IN_EMODE: &[u8] = b"Asset already supported in E-mode.";
pub static ERROR_ASSET_NOT_SUPPORTED_IN_EMODE: &[u8] = b"Asset not supported in E-mode.";

pub static ERROR_EMODE_ASSET_NOT_SUPPORTED_AS_COLLATERAL: &[u8] =
    b"E-mode asset not supported as collateral.";
pub static ERROR_ASSET_NOT_BORROWABLE_IN_ISOLATION: &[u8] = b"Asset not borrowable in isolation.";
pub static ERROR_ASSET_NOT_BORROWABLE_IN_SILOED: &[u8] =
    b"Asset can not be borrowed when in siloed mode, if there are other borrow positions.";
pub static ERROR_ASSET_NOT_SUPPORTED_AS_COLLATERAL: &[u8] = b"Asset not supported as collateral.";
pub static ERROR_INVALID_AGGREGATOR: &[u8] = b"Invalid aggregator.";
pub static ERROR_INVALID_LIQUIDITY_POOL_TEMPLATE: &[u8] = b"Invalid liquidity pool template.";
pub static ERROR_MIX_ISOLATED_COLLATERAL: &[u8] =
    b"Cannot mix isolated collateral with other assets.";
pub static ERROR_CANNOT_USE_EMODE_WITH_ISOLATED_ASSETS: &[u8] =
    b"Cannot use E-Mode with isolated assets.";
pub static ERROR_DEBT_CEILING_REACHED: &[u8] = b"Debt ceiling reached for isolated asset.";
pub static ERROR_ASSET_NOT_BORROWABLE: &[u8] = b"Asset not borrowable.";
pub static ERROR_FLASHLOAN_NOT_ENABLED: &[u8] = b"Flashloan not enabled for this asset.";
pub static ERROR_INVALID_SHARD: &[u8] = b"Invalid shard for flashloan.";
pub static ERROR_SUPPLY_CAP: &[u8] = b"Supply cap reached.";
pub static ERROR_BORROW_CAP: &[u8] = b"Borrow cap reached.";
pub static ERROR_INVALID_EXCHANGE_SOURCE: &[u8] = b"Invalid exchange source.";
pub static ERROR_INVALID_ORACLE_TOKEN_TYPE: &[u8] = b"Invalid oracle token type.";
pub static ERROR_ORACLE_TOKEN_NOT_FOUND: &[u8] = b"Oracle token not found.";
pub static ERROR_UNEXPECTED_FIRST_TOLERANCE: &[u8] = b"Unexpected first tolerance.";
pub static ERROR_UNEXPECTED_LAST_TOLERANCE: &[u8] = b"Unexpected last tolerance.";
pub static ERROR_UNEXPECTED_ANCHOR_TOLERANCES: &[u8] = b"Unexpected anchor tolerances.";
pub static ERROR_PAIR_NOT_ACTIVE: &[u8] = b"Pair not active.";
pub static ERROR_NO_LAST_PRICE_FOUND: &[u8] = b"No last price found.";