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
    #[init]
    fn init(&self, submission_count: usize, oracles: MultiValueEncoded<ManagedAddress>) {
        self.add_oracles(oracles);

        self.require_valid_submission_count(submission_count);
        self.submission_count().set(submission_count);

        self.set_paused(true);
    }

    #[upgrade]
    fn upgrade(&self) {
        self.set_paused(true);
    }

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

    /// Also receives submission count,
    /// so the owner does not have to update it manually with setSubmissionCount before this call
    #[only_owner]
    #[endpoint(removeOracles)]
    fn remove_oracles(&self, submission_count: usize, oracles: MultiValueEncoded<ManagedAddress>) {
        let mut oracle_mapper = self.oracle_status();
        for oracle in oracles {
            let _ = oracle_mapper.remove(&oracle);
        }

        self.set_submission_count(submission_count);
    }

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
