#![no_std]

/// EGLD token identifier constant
pub const EGLD_IDENTIFIER: &str = "EGLD-000000";

/// Minimum first tolerance for oracle price fluctuation (0.50%)
pub const MIN_FIRST_TOLERANCE: u128 = 5_000_000_000_000_000_000;
/// Maximum first tolerance for oracle price fluctuation (50%)
pub const MAX_FIRST_TOLERANCE: u128 = 500_000_000_000_000_000_000;

/// Minimum last tolerance for oracle price fluctuation (1.5%)
pub const MIN_LAST_TOLERANCE: u128 = 12_500_000_000_000_000_000;
/// Maximum last tolerance for oracle price fluctuation (100%)
pub const MAX_LAST_TOLERANCE: u128 = 1_000_000_000_000_000_000_000;

/// EGLD ticker
pub const EGLD_TICKER: &[u8] = b"EGLD";
/// WEGLD ticker
pub const WEGLD_TICKER: &[u8] = b"WEGLD";
/// USD ticker
pub const USD_TICKER: &[u8] = b"USD";

/// Seconds per year
pub const SECONDS_PER_YEAR: u64 = 31_556_926;

pub const SECONDS_PER_MINUTE: u64 = 60;
pub const SECONDS_PER_HOUR: u64 = 3_600;

/// Basis points
pub const BP: u128 = 1_000_000_000_000_000_000_000; // Represents 100%
/// Decimal precision
pub const DECIMAL_PRECISION: usize = 21;
/// Maximum bonus
pub const MAX_BONUS: u128 = 300_000_000_000_000_000_000; // Represents 30% basis points

pub static TOTAL_BORROWED_AMOUNT_STORAGE_KEY: &[u8] = b"borrowed_amount";

pub static TOTAL_SUPPLY_AMOUNT_STORAGE_KEY: &[u8] = b"supplied_amount";

pub static TOTAL_RESERVES_AMOUNT_STORAGE_KEY: &[u8] = b"reserves";

pub static STATE_PAIR_STORAGE_KEY: &[u8] = b"state";

pub static PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY: &[u8] = b"rounds";

pub static PRICE_AGGREGATOR_STATUS_STORAGE_KEY: &[u8] = b"pause_module:paused";