pub static ERROR_NO_LIQUIDATION_BONUS: &[u8] = b"No liquidation bonus present for asset.";
pub static ERROR_NO_LOAN_TO_VALUE: &[u8] = b"No loan-to-value value present for asset.";
pub static ERROR_LOAN_TO_VALUE_ZERO: &[u8] = b"Loan-to-value value cannot be zero.";
pub static ERROR_ASSET_NOT_SUPPORTED: &[u8] = b"Asset not supported.";
pub static ERROR_INSUFFICIENT_COLLATERAL: &[u8] = b"Not enough collateral available for this loan.";
pub static ERROR_INSUFFICIENT_DEPOSIT: &[u8] = b"Not enough tokens deposited for this account.";
pub static ERROR_HEALTH_FACTOR: &[u8] = b"Health not low enough for liquidation.";
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

pub static MAX_THRESHOLD_ERROR_MSG: &[u8] =
    b"Cannot liquidate more than 50% of Liquidatee's position!";

pub static ERROR_COLLATERAL_NOT_FOUND: &[u8] = b"Collateral not found to liquidate.";
pub static ERROR_INSUFFICIENT_COLLATERAL_TO_LIQUIDATE: &[u8] =
    b"Insufficient collateral to liquidate.";

pub static ERROR_PRICE_AGGREGATOR_NOT_SET: &[u8] = b"Price aggregator not set.";
