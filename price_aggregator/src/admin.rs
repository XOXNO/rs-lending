use crate::{
    constants::{SUBMISSION_LIST_MAX_LEN, SUBMISSION_LIST_MIN_LEN},
    errors::{SUBMISSION_LIST_CAPACITY_EXCEEDED_ERROR, SUBMISSION_LIST_MIN_LEN_ERROR},
    structs::OracleStatus,
};

multiversx_sc::imports!();

#[multiversx_sc::module]
pub trait AdminModule:
    crate::storage::StorageModule
    + multiversx_sc_modules::pause::PauseModule
    + crate::utils::UtilsModule
    + crate::views::ViewsModule
    + crate::events::EventsModule
{
    /// Initializes price aggregator with oracle addresses and submission threshold.
    /// Sets initial submission count and pauses contract for configuration.
    /// Validates submission count is within acceptable bounds.
    #[init]
    fn init(&self, submission_count: usize, oracles: MultiValueEncoded<ManagedAddress>) {
        self.add_oracles(oracles);

        self.require_valid_submission_count(submission_count);
        self.submission_count().set(submission_count);

        self.set_paused(true);
    }

    /// Handles contract upgrade by pausing operations.
    /// Ensures safe state during code updates.
    #[upgrade]
    fn upgrade(&self) {
        self.set_paused(true);
    }

    /// Adds new oracle addresses to the whitelist.
    /// Initializes submission statistics for each new oracle.
    /// Skips oracles that are already registered.
    #[only_owner]
    #[endpoint(addOracles)]
    fn add_oracles(&self, oracles: MultiValueEncoded<ManagedAddress>) {
        let mut oracle_mapper = self.oracle_status();
        for oracle in oracles {
            if !oracle_mapper.contains_key(&oracle) {
                let _ = oracle_mapper.insert(
                    oracle.clone(),
                    OracleStatus {
                        total_submissions: 0,
                        accepted_submissions: 0,
                    },
                );
            }
        }
    }

    /// Removes oracle addresses and updates submission count atomically.
    /// Prevents invalid state where submission count exceeds oracle count.
    #[only_owner]
    #[endpoint(removeOracles)]
    fn remove_oracles(&self, submission_count: usize, oracles: MultiValueEncoded<ManagedAddress>) {
        let mut oracle_mapper = self.oracle_status();
        for oracle in oracles {
            let _ = oracle_mapper.remove(&oracle);
        }

        self.set_submission_count(submission_count);
    }

    /// Updates required submission count for consensus.
    /// Validates count is within min/max bounds and doesn't exceed oracle count.
    /// Controls how many oracle submissions trigger price aggregation.
    #[only_owner]
    #[endpoint(setSubmissionCount)]
    fn set_submission_count(&self, submission_count: usize) {
        self.require_valid_submission_count(submission_count);
        require!(
            submission_count <= SUBMISSION_LIST_MAX_LEN,
            SUBMISSION_LIST_CAPACITY_EXCEEDED_ERROR
        );
        require!(
            submission_count >= SUBMISSION_LIST_MIN_LEN,
            SUBMISSION_LIST_MIN_LEN_ERROR
        );
        let oracles = self.get_oracles().len();
        require!(
            submission_count <= oracles,
            SUBMISSION_LIST_CAPACITY_EXCEEDED_ERROR
        );
        self.submission_count().set(submission_count);
    }
}
