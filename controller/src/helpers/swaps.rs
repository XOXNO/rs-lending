use crate::{oracle, storage};

use super::math;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

#[multiversx_sc::module]
pub trait SwapsModule:
    oracle::OracleModule + storage::Storage + math::MathsModule + common_math::SharedMathModule
{
    fn convert_token_from_to(
        &self,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        if to_token == from_token {
            return EgldOrEsdtTokenPayment::new(to_token.clone(), 0, from_amount.clone());
        }
        self.swap_tokens(to_token, from_token, from_amount, caller, args)
    }

    fn swap_tokens(
        self,
        wanted_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ManagedArgBuffer<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        let back_transfers = self
            .tx()
            .to(self.aggregator().get())
            .raw_call(ManagedBuffer::new_from_bytes(b"swap"))
            .arguments_raw(args)
            .egld_or_single_esdt(from_token, 0, from_amount)
            .returns(ReturnsBackTransfersReset)
            .sync_call();

        let mut payments: ManagedVec<EgldOrEsdtTokenPayment<Self::Api>> = ManagedVec::new();

        if back_transfers.total_egld_amount > 0 {
            payments.push(EgldOrEsdtTokenPayment::new(
                EgldOrEsdtTokenIdentifier::egld(),
                0,
                back_transfers.total_egld_amount,
            ));
        }

        for esdt in back_transfers.esdt_payments.iter() {
            payments.push(EgldOrEsdtTokenPayment::new(
                EgldOrEsdtTokenIdentifier::esdt(esdt.token_identifier.clone()),
                esdt.token_nonce,
                esdt.amount.clone(),
            ));
        }

        let mut wanted_result =
            EgldOrEsdtTokenPayment::new(wanted_token.clone(), 0, BigUint::from(0u32));

        let mut refunds = ManagedVec::new();

        for payment in payments {
            if payment.token_identifier == *wanted_token {
                wanted_result.amount += &payment.amount;
            } else {
                refunds.push(payment.clone());
            }
        }

        if refunds.len() > 0 {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        wanted_result
    }
}
