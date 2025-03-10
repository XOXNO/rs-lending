multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::{
    constants::*,
    errors::*,
    median,
    structs::{TimestampedPrice, TokenPair},
};

#[multiversx_sc::module]
pub trait UtilsModule:
    crate::storage::StorageModule
    + crate::events::EventsModule
    + crate::views::ViewsModule
    + multiversx_sc_modules::pause::PauseModule
{
    fn require_is_oracle(&self) {
        let caller = self.blockchain().get_caller();
        require!(
            self.oracle_status().contains_key(&caller),
            ONLY_ORACLES_ALLOWED_ERROR
        );
    }

    fn require_valid_submission_count(&self, submission_count: usize) {
        require!(
            submission_count >= SUBMISSION_LIST_MIN_LEN
                && submission_count <= self.oracle_status().len()
                && submission_count <= SUBMISSION_LIST_MAX_LEN,
            INVALID_SUBMISSION_COUNT_ERROR
        )
    }

    fn submit_unchecked(&self, from: ManagedBuffer, to: ManagedBuffer, price: BigUint) {
        let token_pair = TokenPair { from, to };
        let mut submissions = self
            .submissions()
            .entry(token_pair.clone())
            .or_default()
            .get();

        let first_sub_time_mapper = self.first_submission_timestamp(&token_pair);
        let last_sub_time_mapper = self.last_submission_timestamp(&token_pair);

        let mut round_id = 0u32;
        let wrapped_rounds = self.rounds_new(&token_pair.from, &token_pair.to);
        if !wrapped_rounds.is_empty() {
            round_id = wrapped_rounds.get().round + 1u32;
        }

        let current_timestamp = self.blockchain().get_block_timestamp();
        let mut is_first_submission = false;
        let mut first_submission_timestamp = if submissions.is_empty() {
            first_sub_time_mapper.set(current_timestamp);
            is_first_submission = true;

            current_timestamp
        } else {
            first_sub_time_mapper.get()
        };

        // round was not completed in time, so it's discarded
        if current_timestamp > first_submission_timestamp + MAX_ROUND_DURATION_SECONDS {
            submissions.clear();
            first_sub_time_mapper.set(current_timestamp);
            last_sub_time_mapper.set(current_timestamp);

            first_submission_timestamp = current_timestamp;
            is_first_submission = true;
            self.discard_round_event(&token_pair.from.clone(), &token_pair.to.clone(), round_id)
        }

        let caller = self.blockchain().get_caller();
        let has_caller_already_submitted = submissions.contains_key(&caller);
        let accepted = !has_caller_already_submitted
            && (is_first_submission || current_timestamp >= first_submission_timestamp);
        if accepted {
            submissions.insert(caller.clone(), price.clone());
            last_sub_time_mapper.set(current_timestamp);
            let configured_decimals = self.get_pair_decimals(&token_pair.from, &token_pair.to);

            self.create_new_round(
                token_pair.clone(),
                round_id,
                submissions,
                configured_decimals,
            );
            self.add_submission_event(
                &token_pair.from.clone(),
                &token_pair.to.clone(),
                round_id,
                &price,
            );
        } else {
            self.emit_discard_submission_event(
                &token_pair,
                round_id,
                current_timestamp,
                first_submission_timestamp,
                has_caller_already_submitted,
            );
        }

        self.oracle_status()
            .entry(self.blockchain().get_caller())
            .and_modify(|oracle_status| {
                oracle_status.accepted_submissions += accepted as u64;
                oracle_status.total_submissions += 1;
            });
    }

    fn require_valid_submission_timestamp(&self, submission_timestamp: u64) {
        let current_timestamp = self.blockchain().get_block_timestamp();
        require!(
            submission_timestamp <= current_timestamp,
            TIMESTAMP_FROM_FUTURE_ERROR
        );
        require!(
            current_timestamp - submission_timestamp <= FIRST_SUBMISSION_TIMESTAMP_MAX_DIFF_SECONDS,
            FIRST_SUBMISSION_TOO_OLD_ERROR
        );
    }

    fn create_new_round(
        &self,
        token_pair: TokenPair<Self::Api>,
        round_id: u32,
        mut submissions: MapMapper<ManagedAddress, BigUint>,
        asset_decimals: u8,
    ) {
        let submissions_len = submissions.len();
        if submissions_len >= self.submission_count().get() {
            require!(
                submissions_len <= SUBMISSION_LIST_MAX_LEN,
                SUBMISSION_LIST_CAPACITY_EXCEEDED_ERROR
            );

            let mut submissions_vec = ArrayVec::<BigUint, SUBMISSION_LIST_MAX_LEN>::new();
            for submission_value in submissions.values() {
                submissions_vec.push(submission_value);
            }

            let price_result = median::calculate(submissions_vec.as_mut_slice());
            let price_opt = price_result.unwrap_or_else(|err| sc_panic!(err.as_bytes()));
            let price = price_opt.unwrap_or_else(|| sc_panic!(NO_SUBMISSIONS_ERROR));
            let feed = TimestampedPrice {
                price,
                timestamp: self.blockchain().get_block_timestamp(),
                asset_decimals,
                round: round_id,
            };

            submissions.clear();
            self.first_submission_timestamp(&token_pair).clear();
            self.last_submission_timestamp(&token_pair).clear();
            self.rounds_new(&token_pair.from, &token_pair.to).set(&feed);

            self.emit_new_round_event(&token_pair, round_id, &feed);
        }
    }

    fn clear_submissions(&self, token_pair: &TokenPair<Self::Api>) {
        if let Some(mut pair_submission_mapper) = self.submissions().get(token_pair) {
            pair_submission_mapper.clear();
        }
        self.first_submission_timestamp(token_pair).clear();
        self.last_submission_timestamp(token_pair).clear();
    }
}
