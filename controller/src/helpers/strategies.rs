use crate::{oracle, storage};
use common_events::TokenOutData;
use common_structs::{FeeMoment, FeeToken, SwapStep, TokenInData};

use super::math;

multiversx_sc::imports!();
multiversx_sc::derive_imports!();

pub struct MathHelpers;

#[type_abi]
#[derive(TopDecode, NestedDecode)]
pub struct ArdaSwapArgs<M: ManagedTypeApi> {
    pub token_in_data: TokenInData<M>,
    pub swap_path_a: ManagedVec<M, SwapStep<M>>,
    pub swap_path_b: ManagedVec<M, SwapStep<M>>,
    pub token_out_data: TokenOutData<M>,
    pub min_amount_out: BigUint<M>,
    pub fee_moment: FeeMoment,
    pub fee_token: FeeToken,
}

#[multiversx_sc::module]
pub trait StrategiesModule:
    oracle::OracleModule + storage::Storage + math::MathsModule + common_math::SharedMathModule
{
    fn convert_token_from_to(
        &self,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        steps: ArdaSwapArgs<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        if to_token == from_token {
            return EgldOrEsdtTokenPayment::new(to_token.clone(), 0, from_amount.clone());
        }
        self.swap_tokens(to_token, from_token, from_amount, caller, steps)
    }

    fn swap_tokens(
        self,
        wanted_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        caller: &ManagedAddress,
        args: ArdaSwapArgs<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        // Set up the aggregator contract call
        let back_transfers = self
            .arda_price_proxy(self.aggregator().get())
            .swap(
                args.token_in_data.clone(),
                args.swap_path_a.clone(),
                args.swap_path_b.clone(),
                args.token_out_data.clone(),
                args.min_amount_out.clone(),
                args.fee_moment,
                args.fee_token,
            )
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

    #[proxy]
    fn arda_price_proxy(&self, sc_address: ManagedAddress) -> arda_price_proxy::ProxyTo<Self::Api>;
}

mod arda_price_proxy {
    multiversx_sc::imports!();
    use common_structs::{FeeMoment, FeeToken, SwapPath, TokenInData, TokenOutData};

    #[multiversx_sc::proxy]
    pub trait ArdaPriceContract {
        #[payable("*")]
        #[endpoint(swap)]
        fn swap(
            &self,
            token_in_data: TokenInData<Self::Api>,
            swap_path_a: SwapPath<Self::Api>,
            swap_path_b: SwapPath<Self::Api>,
            token_out_data: TokenOutData<Self::Api>,
            min_amount_out: BigUint,
            fee_moment: FeeMoment,
            fee_token: FeeToken,
        );
    }
}
