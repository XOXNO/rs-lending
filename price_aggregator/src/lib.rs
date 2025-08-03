#![no_std]

multiversx_sc::imports!();

pub mod admin;
pub mod constants;
pub mod errors;
pub mod events;
pub mod median;
pub mod storage;
pub mod structs;
pub mod utils;
pub mod views;

#[multiversx_sc::contract]
pub trait PriceAggregator:
    multiversx_sc_modules::pause::PauseModule
    + events::EventsModule
    + utils::UtilsModule
    + storage::StorageModule
    + views::ViewsModule
    + admin::AdminModule
{
    /// Submits a single price feed from oracle for token pair.
    /// Validates oracle status and timestamp before processing submission.
    /// Triggers new round creation when submission threshold is met.
    #[endpoint(submit)]
    fn submit(
        &self,
        from: ManagedBuffer,
        to: ManagedBuffer,
        submission_timestamp: u64,
        price: BigUint,
    ) {
        self.require_not_paused();
        self.require_is_oracle();

        self.require_valid_submission_timestamp(submission_timestamp);

        self.submit_unchecked(from, to, price);
    }

    /// Submits multiple price feeds in a single transaction for gas efficiency.
    /// Each submission is validated independently before processing.
    /// Enables oracles to update multiple token pairs atomically.
    #[endpoint(submitBatch)]
    fn submit_batch(
        &self,
        submissions: MultiValueEncoded<MultiValue4<ManagedBuffer, ManagedBuffer, u64, BigUint>>,
    ) {
        self.require_not_paused();
        self.require_is_oracle();

        for (from, to, submission_timestamp, price) in submissions
            .into_iter()
            .map(|submission| submission.into_tuple())
        {
            self.require_valid_submission_timestamp(submission_timestamp);

            self.submit_unchecked(from, to, price);
        }
    }
}
