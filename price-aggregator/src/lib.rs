#![no_std]

multiversx_sc::imports!();

pub mod admin;
pub mod events;
pub mod median;
pub mod structs;
pub mod storage;
pub mod utils;
pub mod views;
pub mod constants;
pub mod errors;

#[multiversx_sc::contract]
pub trait PriceAggregator:
    multiversx_sc_modules::pause::PauseModule
    + events::EventsModule
    + utils::UtilsModule
    + storage::StorageModule
    + views::ViewsModule
    + admin::AdminModule
{
    #[endpoint]
    fn submit(
        &self,
        from: ManagedBuffer,
        to: ManagedBuffer,
        submission_timestamp: u64,
        price: BigUint,
        decimals: u8,
    ) {
        self.require_not_paused();
        self.require_is_oracle();

        self.require_valid_submission_timestamp(submission_timestamp);

        self.check_decimals(&from, &to, decimals);

        self.submit_unchecked(from, to, price, decimals);
    }

    #[endpoint(submitBatch)]
    fn submit_batch(
        &self,
        submissions: MultiValueEncoded<MultiValue5<ManagedBuffer, ManagedBuffer, u64, BigUint, u8>>,
    ) {
        self.require_not_paused();
        self.require_is_oracle();

        for (from, to, submission_timestamp, price, decimals) in submissions
            .into_iter()
            .map(|submission| submission.into_tuple())
        {
            self.require_valid_submission_timestamp(submission_timestamp);

            self.check_decimals(&from, &to, decimals);

            self.submit_unchecked(from, to, price, decimals);
        }
    }
}
