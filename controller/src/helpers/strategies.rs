use common_constants::WAD;
use common_events::{ExchangeSource, OracleProvider, OracleType};

use crate::{
    aggregator::{AggregatorContractProxy, AggregatorStep, TokenAmount},
    lxoxno_proxy, oracle, proxy_xexchange_pair, storage, wegld_proxy, xegld_proxy,
};

use super::math;

multiversx_sc::imports!();

pub struct MathHelpers;

#[multiversx_sc::module]
pub trait StrategiesModule:
    oracle::OracleModule
    + storage::LendingStorageModule
    + math::MathsModule
    + common_math::SharedMathModule
{
    fn get_xegld(&self, egld_amount: &BigUint, sc_address: &ManagedAddress) -> EsdtTokenPayment {
        let result = self
            .tx()
            .to(sc_address)
            .typed(xegld_proxy::LiquidStakingProxy)
            .delegate(OptionalValue::<ManagedAddress>::None)
            .egld(egld_amount)
            .returns(ReturnsResult)
            .sync_call();

        require!(result.is_some(), "xEGLD not minted!");

        result.unwrap()
    }

    fn get_lxoxno(
        &self,
        xoxno_amount: &BigUint,
        xoxno_token: &EgldOrEsdtTokenIdentifier,
        sc_address: &ManagedAddress,
    ) -> EsdtTokenPayment {
        let lxoxno = self
            .tx()
            .to(sc_address)
            .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
            .delegate(OptionalValue::<ManagedAddress>::None)
            .single_esdt(&xoxno_token.as_esdt_option().unwrap(), 0, xoxno_amount)
            .returns(ReturnsResult)
            .sync_call();

        lxoxno
    }

    fn split_collateral(
        &self,
        lp_amount: &BigUint,
        lp_token: &TokenIdentifier,
        sc_address: &ManagedAddress,
    ) -> MultiValue2<EsdtTokenPayment, EsdtTokenPayment> {
        self.tx()
            .to(sc_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .remove_liquidity(BigUint::from(1u32), BigUint::from(1u32))
            .single_esdt(lp_token, 0, lp_amount)
            .returns(ReturnsResult)
            .sync_call()
    }

    fn create_lp_token(
        &self,
        sc_address: &ManagedAddress,
        first_token: &EgldOrEsdtTokenIdentifier,
        second_token: &EgldOrEsdtTokenIdentifier,
        first_token_amount: BigUint,
        secont_token_amount: BigUint,
    ) -> (EgldOrEsdtTokenIdentifier, BigUint) {
        let mut payments = ManagedVec::new();
        payments.push(EsdtTokenPayment::new(
            first_token.clone().into_esdt_option().unwrap(),
            0,
            first_token_amount,
        ));
        payments.push(EsdtTokenPayment::new(
            second_token.clone().into_esdt_option().unwrap(),
            0,
            secont_token_amount,
        ));

        let (lp_received, first, second) = self
            .tx()
            .to(sc_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .add_liquidity(BigUint::from(1u32), BigUint::from(1u32))
            .multi_esdt(payments)
            .returns(ReturnsResult)
            .sync_call()
            .into_tuple();

        let caller = self.blockchain().get_caller();

        self.tx().to(&caller).esdt(first).transfer_if_not_empty();
        self.tx().to(&caller).esdt(second).transfer_if_not_empty();

        (
            EgldOrEsdtTokenIdentifier::esdt(lp_received.token_identifier.clone()),
            lp_received.amount,
        )
    }

    // Always the collateral token is the LSD token while debt token is the root main token of the LSD: such as xEGLD with EGLD or LXOXNO with XOXNO
    fn process_payment_to_collateral(
        &self,
        payment: &EgldOrEsdtTokenPayment,
        oracle_payment: &OracleProvider<Self::Api>,
        wanted_collateral: &EgldOrEsdtTokenIdentifier,
        oracle_collateral: &OracleProvider<Self::Api>,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        if &payment.token_identifier == wanted_collateral {
            return payment.clone();
        } else {
            self.convert_token_from_to(
                oracle_collateral,
                wanted_collateral,
                &payment.token_identifier,
                &payment.amount,
                oracle_payment,
                steps,
                limits,
            )
        }
    }

    fn process_flash_loan_to_collateral(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        init_collateral_amount: &BigUint,
        to_provider: &OracleProvider<Self::Api>,
        from_provider: &OracleProvider<Self::Api>,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        let mut extra_collateral = self.convert_token_from_to(
            to_provider,
            to_token,
            from_token,
            from_amount,
            from_provider,
            steps,
            limits,
        );
        extra_collateral.amount += init_collateral_amount;
        extra_collateral
    }

    fn convert_token_from_to(
        &self,
        to_provider: &OracleProvider<Self::Api>,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        from_provider: &OracleProvider<Self::Api>,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        match (
            from_provider.oracle_type.clone(),
            to_provider.oracle_type.clone(),
        ) {
            (OracleType::Normal, OracleType::Derived) => {
                if from_token == &to_provider.base_token_id {
                    self.convert_to_lsd(from_token, &from_amount, to_provider)
                } else {
                    self.swap_tokens(to_token, from_token, from_amount, steps, limits)
                }
            }
            (OracleType::Normal, OracleType::Normal) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Normal, OracleType::Lp) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Derived, OracleType::Normal) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Derived, OracleType::Derived) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Derived, OracleType::Lp) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Lp, OracleType::Normal) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Lp, OracleType::Derived) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            (OracleType::Lp, OracleType::Lp) => {
                self.swap_tokens(to_token, from_token, from_amount, steps, limits)
            }
            _ => sc_panic!("Unsupported conversion type"),
        }
        // if from_provider.token_type == OracleType::Normal {
        //     if to_provider.token_type == OracleType::Derived {
        //         require!(
        //             from_token.eq(&to_provider.first_token_id),
        //             "Impossible to convert received token to LSD"
        //         );
        //         return self.convert_to_lsd(from_token, &from_amount, to_provider);
        //     } else if to_provider.token_type == OracleType::Lp {
        //         let wegld = ManagedBuffer::from(WEGLD_TICKER);
        //         // EGLD
        //         let from_is_egld = from_token.is_egld();
        //         let first_token_is_wegld =
        //             to_provider.first_token_id.clone().unwrap_esdt().ticker() == wegld;
        //         let second_token_is_wegld =
        //             to_provider.second_token_id.clone().unwrap_esdt().ticker() == wegld;

        //         let is_wegld_with_egld =
        //             from_is_egld && (first_token_is_wegld || second_token_is_wegld);

        //         let is_first_token = from_token == &to_provider.first_token_id
        //             || (from_is_egld && first_token_is_wegld);
        //         // Second token should be LSD token
        //         let is_second_token = from_token == &to_provider.second_token_id
        //             || (from_is_egld && second_token_is_wegld);

        //         require!(
        //             is_first_token || is_second_token || is_wegld_with_egld,
        //             "Impossible to convert the received token to a LP position"
        //         );

        //         // Even if we do not have a market for WEGLD we have a token oracle configuration as a clone of EGLD one
        //         let lsd_token_config = if is_first_token {
        //             self.token_oracle(&to_provider.second_token_id).get()
        //         } else {
        //             self.token_oracle(&to_provider.first_token_id).get()
        //         };

        //         // The half token of LP should be a LSD token and the original LSD token has to be the initial normal payment token.
        //         // Example xEGLD/EGLD -> Payment in EGLD, then LP has xEGLD with EGLD where the xEGLD is derived from the payment of EGLD
        //         require!(
        //             lsd_token_config.token_type == OracleType::Derived
        //                 && &lsd_token_config.first_token_id == from_token,
        //             "We do not allow Strategys for LPs without LSD pairs"
        //         );

        //         let lsd_ratio = self.get_lsd_ratio(&lsd_token_config);
        //         let (first_token_reserve, second_token_reserve, _) = self
        //             .tx()
        //             .to(&to_provider.contract_address)
        //             .typed(proxy_xexchange_pair::PairProxy)
        //             .get_reserves_and_total_supply()
        //             .returns(ReturnsResult)
        //             .sync_call_readonly()
        //             .into_tuple();

        //         let (original_remaining, to_convert_in_lsd) = if is_first_token {
        //             self.calculate_fix_lp_proportions(
        //                 from_amount,
        //                 &lsd_ratio,
        //                 &first_token_reserve,
        //                 &second_token_reserve,
        //             )
        //         } else {
        //             self.calculate_fix_lp_proportions(
        //                 from_amount,
        //                 &lsd_ratio,
        //                 &second_token_reserve,
        //                 &first_token_reserve,
        //             )
        //         };

        //         require!(
        //             &(&original_remaining + &to_convert_in_lsd) == from_amount,
        //             "Amount split has dust {}, {}, {}",
        //             original_remaining,
        //             to_convert_in_lsd,
        //             from_amount
        //         );

        //         let converted_lsd =
        //             self.convert_to_lsd(from_token, &to_convert_in_lsd, &lsd_token_config);

        //         if is_wegld_with_egld {
        //             self.wrap_egld(&original_remaining);
        //         }

        //         sc_panic!(
        //             "First token {}, amount {}, Second token {}, amount {}",
        //             to_provider.first_token_id,
        //             converted_lsd.amount,
        //             to_provider.second_token_id,
        //             original_remaining
        //         );

        //         let (lp_token, lp_amount) = self.create_lp_token(
        //             &to_provider.contract_address,
        //             &to_provider.first_token_id,
        //             &to_provider.second_token_id,
        //             converted_lsd.amount,
        //             original_remaining,
        //         );

        //         require!(
        //             &lp_token == to_token,
        //             "The resulted LP token is not matching the required collateral token!"
        //         );

        //         EgldOrEsdtTokenPayment::new(lp_token, 0, lp_amount)
        //     } else {
        //         sc_panic!("Strategy is not possible due to collateral token type!");
        //     }
        // } else if from_provider.token_type == OracleType::Lp {
        //     if to_provider.token_type == OracleType::Derived {
        //         let (first_token, second_token) = self
        //             .split_collateral(
        //                 from_amount,
        //                 &from_token.as_esdt_option().unwrap(),
        //                 &from_provider.contract_address,
        //             )
        //             .into_tuple();

        //         let (lsd_payment, original_payment) =
        //             if first_token.token_identifier == to_token.clone().unwrap_esdt() {
        //                 (first_token, second_token)
        //             } else {
        //                 (second_token, first_token)
        //             };

        //         let is_wegld =
        //             original_payment.token_identifier.ticker() == ManagedBuffer::from(WEGLD_TICKER);

        //         if is_wegld {
        //             self.unwrap_wegld(&original_payment.amount, &original_payment.token_identifier);
        //         }

        //         require!(
        //             &lsd_payment.token_identifier == to_token,
        //             "The LP token is not part of the LSD original token {} {}",
        //             (lsd_payment.token_identifier),
        //             (to_token)
        //         );

        //         require!(
        //             &original_payment.token_identifier == &to_provider.first_token_id
        //                 || (is_wegld && to_provider.first_token_id.is_egld()),
        //             "The LP token is not part of the LSD original token {} {}",
        //             (original_payment.token_identifier),
        //             (to_provider.first_token_id)
        //         );

        //         let mut new_lsd_payment = self.convert_to_lsd(
        //             &to_provider.first_token_id,
        //             &original_payment.amount,
        //             to_provider,
        //         );

        //         new_lsd_payment.amount += lsd_payment.amount;

        //         new_lsd_payment
        //     } else {
        //         sc_panic!("Strategy not implemented yet!");
        //     }
        // } else {
        //     sc_panic!("Strategy not implemented yet!");
        // }
    }

    fn convert_to_lsd(
        &self,
        token: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        oracle_collateral: &OracleProvider<Self::Api>,
    ) -> EgldOrEsdtTokenPayment {
        if oracle_collateral.exchange_source == ExchangeSource::XEGLD {
            self.get_xegld(amount, &oracle_collateral.oracle_contract_address)
                .into()
        } else if oracle_collateral.exchange_source == ExchangeSource::LXOXNO {
            self.get_lxoxno(amount, token, &oracle_collateral.oracle_contract_address)
                .into()
        } else {
            sc_panic!("Strategy is not possible due to LSD conversion not existing!");
        }
    }

    fn get_lsd_ratio(&self, oracle_collateral: &OracleProvider<Self::Api>) -> BigUint {
        if oracle_collateral.exchange_source == ExchangeSource::XEGLD {
            self.tx()
                .to(&oracle_collateral.oracle_contract_address)
                .typed(xegld_proxy::LiquidStakingProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else if oracle_collateral.exchange_source == ExchangeSource::LXOXNO {
            self.tx()
                .to(&oracle_collateral.oracle_contract_address)
                .typed(lxoxno_proxy::RsLiquidXoxnoProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else {
            sc_panic!("Strategy is not possible due to LSD conversion not existing!");
        }
    }

    fn calculate_fix_lp_proportions(
        &self,
        payment_amount: &BigUint, // Amount of EGLD to be split (18 asset_decimals)
        lsd_ratio: &BigUint,      // 1 xEGLD = lsd_ratio EGLD (18 asset_decimals)
        first_token_reserve: &BigUint, // EGLD reserve (18 asset_decimals)
        second_token_reserve: &BigUint, // xEGLD reserve (18 asset_decimals)
    ) -> (BigUint, BigUint) {
        // p is the xEGLD reserve expressed in EGLD value:
        // TODO: Verify if / WAD make sense for other token LPs
        let p = (second_token_reserve * lsd_ratio) / &BigUint::from(WAD);
        // To add liquidity you must deposit tokens in the same ratio as the pool:
        //   (EGLD deposit) : (xEGLD deposit in EGLD value) = first_token_reserve : p
        // Let x = amount to convert (which becomes the xEGLD deposit in EGLD value)
        // and (payment_amount – x) = EGLD you keep.
        // Then (payment_amount – x) : x = first_token_reserve : p.
        // Solving for x gives:
        //
        //   x = payment_amount * p / (first_token_reserve + p)
        //
        let convert_value = (payment_amount * &p) / (first_token_reserve + &p);
        let remain_value = payment_amount - &convert_value;
        (remain_value, convert_value)
    }

    fn swap_tokens(
        self,
        to: &EgldOrEsdtTokenIdentifier,
        from: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        require!(
            steps.is_some() && limits.is_some(),
            "Steps and limits are required"
        );
        let call = self
            .tx()
            .to(self.aggregator().get())
            .typed(AggregatorContractProxy);

        if from.is_esdt() {
            let second_call = call
                .aggregate_esdt(
                    steps.unwrap(),
                    limits.unwrap(),
                    to.is_egld(),
                    OptionalValue::<ManagedAddress>::None,
                )
                .egld_or_single_esdt(&from, 0, amount);

            let result = if to.is_egld() {
                let amount = second_call.returns(ReturnsBackTransfersEGLD).sync_call();
                EgldOrEsdtTokenPayment::new(EgldOrEsdtTokenIdentifier::egld(), 0, amount)
            } else {
                second_call
                    .returns(ReturnsBackTransfersSingleESDT)
                    .sync_call()
                    .into()
            };

            result
        } else {
            let result = call
                .aggregate_egld(
                    steps.unwrap(),
                    limits.unwrap(),
                    OptionalValue::<ManagedAddress>::None,
                )
                .egld(amount)
                .returns(ReturnsBackTransfersSingleESDT)
                .sync_call();

            result.into_multi_egld_or_esdt_payment()
        }
    }

    fn wrap_egld(&self, amount: &BigUint) {
        self.tx()
            .to(self.wegld_wrapper().get())
            .typed(wegld_proxy::EgldEsdtSwapProxy)
            .wrap_egld()
            .egld(amount)
            .sync_call();
    }

    fn unwrap_wegld(&self, amount: &BigUint, token: &TokenIdentifier) {
        self.tx()
            .to(self.wegld_wrapper().get())
            .typed(wegld_proxy::EgldEsdtSwapProxy)
            .unwrap_egld()
            .single_esdt(token, 0, amount)
            .sync_call();
    }
}
