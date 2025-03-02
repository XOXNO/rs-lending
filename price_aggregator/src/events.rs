multiversx_sc::imports!();
multiversx_sc::derive_imports!();

use crate::structs::{DiscardSubmissionEvent, NewRoundEvent, TimestampedPrice, TokenPair};

#[multiversx_sc::module]
pub trait EventsModule {
    fn emit_new_round_event(
        &self,
        token_pair: &TokenPair<Self::Api>,
        round_id: u32,
        feed: &TimestampedPrice<Self::Api>,
    ) {
        let epoch = self.blockchain().get_block_epoch();

        self.new_round_event(
            &token_pair.from.clone(),
            &token_pair.to.clone(),
            round_id,
            &NewRoundEvent {
                price: feed.price.clone(),
                timestamp: feed.timestamp,
                asset_decimals: feed.asset_decimals,
                block: self.blockchain().get_block_nonce(),
                epoch,
            },
        )
    }

    #[event("new_round")]
    fn new_round_event(
        &self,
        #[indexed] from: &ManagedBuffer,
        #[indexed] to: &ManagedBuffer,
        #[indexed] round: u32,
        new_round_event: &NewRoundEvent<Self::Api>,
    );

    fn emit_discard_submission_event(
        &self,
        token_pair: &TokenPair<Self::Api>,
        round_id: u32,
        submission_timestamp: u64,
        first_submission_timestamp: u64,
        has_caller_already_submitted: bool,
    ) {
        self.discard_submission_event(
            &token_pair.from.clone(),
            &token_pair.to.clone(),
            round_id,
            &DiscardSubmissionEvent {
                submission_timestamp,
                first_submission_timestamp,
                has_caller_already_submitted,
            },
        )
    }

    #[event("discard_submission")]
    fn discard_submission_event(
        &self,
        #[indexed] from: &ManagedBuffer,
        #[indexed] to: &ManagedBuffer,
        #[indexed] round: u32,
        discard_submission_event: &DiscardSubmissionEvent,
    );

    #[event("discard_round")]
    fn discard_round_event(
        &self,
        #[indexed] from: &ManagedBuffer,
        #[indexed] to: &ManagedBuffer,
        #[indexed] round: u32,
    );

    #[event("add_submission")]
    fn add_submission_event(
        &self,
        #[indexed] from: &ManagedBuffer,
        #[indexed] to: &ManagedBuffer,
        #[indexed] round: u32,
        price: &BigUint,
    );
}
