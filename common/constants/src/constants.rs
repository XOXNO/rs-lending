#![no_std]

/// Minimum first tolerance for oracle price fluctuation (0.50%)
pub const MIN_FIRST_TOLERANCE: u128 = 50;
/// Maximum first tolerance for oracle price fluctuation (50%)
pub const MAX_FIRST_TOLERANCE: u128 = 5_000;

/// Minimum last tolerance for oracle price fluctuation (1.5%)
pub const MIN_LAST_TOLERANCE: u128 = 150;
/// Maximum last tolerance for oracle price fluctuation (100%)
pub const MAX_LAST_TOLERANCE: u128 = 10_000;

pub const EGLD_TICKER: &[u8] = b"EGLD";
pub const WEGLD_TICKER: &[u8] = b"WEGLD";
pub const USD_TICKER: &[u8] = b"USD";

/// Seconds per year
pub const SECONDS_PER_YEAR: u64 = 31_556_926;

pub const SECONDS_PER_MINUTE: u64 = 60;
pub const SECONDS_PER_HOUR: u64 = 3_600;


pub const RAY: u128 = 1_000_000_000_000_000_000_000_000_000;
pub const RAY_PRECISION: usize = 27;

/// Basis points for 1 EGLD which is the base price for all assets
pub const WAD: u128 = 1_000_000_000_000_000_000; // Represents 1 EGLD
pub const WAD_PRECISION: usize = 18;

pub const BPS: usize = 10_000; // 100%
pub const BPS_PRECISION: usize= 4;


/// Maximum bonus
pub const MAX_BONUS: u128 = 3_000; // Represents 30% basis points

pub static TOTAL_BORROWED_AMOUNT_STORAGE_KEY: &[u8] = b"borrowed_amount";

pub static TOTAL_SUPPLY_AMOUNT_STORAGE_KEY: &[u8] = b"supplied_amount";

pub static TOTAL_RESERVES_AMOUNT_STORAGE_KEY: &[u8] = b"reserves";

pub static STATE_PAIR_STORAGE_KEY: &[u8] = b"state";

pub static PRICE_AGGREGATOR_ROUNDS_STORAGE_KEY: &[u8] = b"rounds";

pub static PRICE_AGGREGATOR_STATUS_STORAGE_KEY: &[u8] = b"pause_module:paused";
