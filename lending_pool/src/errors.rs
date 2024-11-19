pub static ERROR_NO_LIQUIDATION_BONUS: &[u8] = b"No liquidation bonus present for asset.";
pub static ERROR_NO_LOAN_TO_VALUE: &[u8] = b"No loan-to-value value present for asset.";
pub static ERROR_LOAN_TO_VALUE_ZERO: &[u8] = b"Loan-to-value value cannot be zero.";
pub static ERROR_ASSET_NOT_SUPPORTED: &[u8] = b"Asset not supported.";
pub static ERROR_TOKEN_PRICE_FETCH: &[u8] = b"Failed to get token price.";
pub static ERROR_INSUFFICIENT_COLLATERAL: &[u8] = b"Not enough collateral available for this loan.";
pub static ERROR_INSUFFICIENT_DEPOSIT: &[u8] = b"Not enough tokens deposited for this account.";
pub static ERROR_COLLECTION_NOT_ALLOWED: &[u8] = b"Collection is not allowed as collateral.";
pub static ERROR_HEALTH_FACTOR: &[u8] = b"Health not low enough for liquidation.";
pub static ERROR_TOKEN_MISMATCH: &[u8] = b"Token sent is not the same as the liquidation token.";
pub static ERROR_INSUFFICIENT_LIQUIDATION: &[u8] = b"Insufficient funds for liquidation.";
pub static ERROR_MAX_THRESHOLD: &[u8] = b"Liquidation threshold cannot exceed maximum.";
pub static ERROR_TOKEN_NOT_AVAILABLE: &[u8] = b"Tokens are not available for this account.";
pub static ERROR_NO_COLLATERAL_TOKEN: &[u8] = b"Liquidatee user doesn't have this token as collateral.";

// From router.rs
pub static ERROR_ASSET_ALREADY_SUPPORTED: &[u8] = b"Asset already supported.";
pub static ERROR_INVALID_TICKER: &[u8] = b"Invalid ticker provided.";
pub static ERROR_NO_POOL_FOUND: &[u8] = b"No pool found for this asset.";
pub static ERROR_NO_POOL_ADDRESS: &[u8] = b"No pool address for asset.";

// From factory.rs
pub static ERROR_TEMPLATE_EMPTY: &[u8] = b"Liquidity pool contract template is empty.";

// From utils.rs
pub static ERROR_EGLD_PRICE_FETCH: &[u8] = b"Failed to get EGLD price.";
pub static ERROR_TOKEN_TICKER_FETCH: &[u8] = b"Failed to get token ticker.";

// From lib.rs
pub static ERROR_ZERO_AMOUNT: &[u8] = b"Amount must be greater than zero.";
pub static ERROR_ZERO_ADDRESS: &[u8] = b"Address cannot be zero.";
pub static ERROR_NOT_IN_MARKET: &[u8] = b"Account not in the market.";
pub static ERROR_INVALID_TOKEN: &[u8] = b"Invalid account token.";
pub static ERROR_MIN_2_TOKENS: &[u8] = b"Minimum 2 tokens required for this operation.";

// New constants from various files
pub static ERROR_INVALID_ACCOUNT_TOKEN: &[u8] = b"Invalid account token provided";
pub static ERROR_ACCOUNT_NOT_IN_MARKET: &[u8] = b"Account is not in the market";
pub static ERROR_INVALID_POOL_ADDRESS: &[u8] = b"Invalid pool address";
pub static ERROR_POOL_NOT_ALLOWED: &[u8] = b"Pool not allowed";
pub static ERROR_INVALID_ASSET_PRICE: &[u8] = b"Invalid asset price";
pub static ERROR_INVALID_DEPOSIT_AMOUNT: &[u8] = b"Invalid deposit amount";
pub static ERROR_INVALID_BORROW_AMOUNT: &[u8] = b"Invalid borrow amount";
pub static ERROR_INVALID_REPAY_AMOUNT: &[u8] = b"Invalid repay amount";
pub static ERROR_INSUFFICIENT_LIQUIDITY: &[u8] = b"Insufficient liquidity in pool";
pub static ERROR_INVALID_LIQUIDATION_AMOUNT: &[u8] = b"Invalid liquidation amount";
pub static ERROR_INVALID_PARAMETER: &[u8] = b"Invalid parameter provided";
pub static ERROR_UNAUTHORIZED_ACCESS: &[u8] = b"Unauthorized access";
pub static ERROR_INVALID_STATE: &[u8] = b"Invalid contract state";
pub static ERROR_OPERATION_NOT_ALLOWED: &[u8] = b"Operation not allowed";