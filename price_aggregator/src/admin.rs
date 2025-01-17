use crate::structs::{OracleStatus, TokenPair};

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

        self.require_valid_submission_count(submission_count);
        self.submission_count().set(submission_count);
    }

    #[only_owner]
    #[endpoint(setSubmissionCount)]
    fn set_submission_count(&self, submission_count: usize) {
        self.require_valid_submission_count(submission_count);
        self.submission_count().set(submission_count);
    }

    #[only_owner]
    #[endpoint(setPairDecimals)]
    fn set_pair_decimals(&self, from: ManagedBuffer, to: ManagedBuffer, decimals: u8) {
        let pair_decimals_mapper = self.pair_decimals(&from, &to);
        if !pair_decimals_mapper.is_empty() {
            self.require_paused();
        }
        pair_decimals_mapper.set(Some(decimals));
        let pair: TokenPair<<Self as ContractBase>::Api> = TokenPair { from, to };
        self.clear_submissions(&pair);
    }
}
