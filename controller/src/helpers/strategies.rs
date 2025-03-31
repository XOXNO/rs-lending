use common_constants::{BPS, WEGLD_TICKER};
use common_errors::{ERROR_WRONG_TOKEN, ERROR_ZERO_AMOUNT};
use common_structs::{ExchangeSource, OracleProvider, OracleType};

use crate::{
    oracle,
    proxy_ashswap::{AggregatorContractProxy, AggregatorStep, TokenAmount},
    proxy_lxoxno, proxy_wegld, proxy_xegld, proxy_xexchange_pair, storage,
};

use super::math;

multiversx_sc::imports!();

pub struct MathHelpers;

#[multiversx_sc::module]
pub trait StrategiesModule:
    oracle::OracleModule + storage::Storage + math::MathsModule + common_math::SharedMathModule
{
    fn get_xegld(&self, egld_amount: &BigUint, sc_address: &ManagedAddress) -> EsdtTokenPayment {
        let result = self
            .tx()
            .to(sc_address)
            .typed(proxy_xegld::LiquidStakingProxy)
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
            .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
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
        base_token: &EgldOrEsdtTokenIdentifier,
        quote_token: &EgldOrEsdtTokenIdentifier,
        base_token_amount: BigUint,
        quote_token_amount: BigUint,
        lp_token: &EgldOrEsdtTokenIdentifier,
    ) -> EgldOrEsdtTokenPayment {
        let mut payments = ManagedVec::new();

        payments.push(EsdtTokenPayment::new(
            base_token.clone().into_esdt_option().unwrap(),
            0,
            base_token_amount,
        ));
        payments.push(EsdtTokenPayment::new(
            quote_token.clone().into_esdt_option().unwrap(),
            0,
            quote_token_amount,
        ));

        let back_transfers = self
            .tx()
            .to(sc_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .add_liquidity(BigUint::from(1u32), BigUint::from(1u32))
            .multi_esdt(payments)
            .returns(ReturnsBackTransfersReset)
            .sync_call();

        let caller = self.blockchain().get_caller();
        if back_transfers.total_egld_amount > 0 {
            self.tx()
                .to(&caller)
                .egld(back_transfers.total_egld_amount)
                .transfer_if_not_empty();
        }
        let mut lp_received = EgldOrEsdtTokenPayment::new(lp_token.clone(), 0, BigUint::from(0u32));
        for esdt in &back_transfers.esdt_payments {
            if esdt.token_identifier == *lp_token {
                lp_received.amount += &esdt.amount;
            } else {
                self.tx()
                    .to(&caller)
                    .esdt(esdt.clone())
                    .transfer_if_not_empty();
            }
        }

        lp_received
    }

    fn get_wegld_token_id(&self) -> ManagedBuffer {
        ManagedBuffer::from(WEGLD_TICKER)
    }

    fn get_lp_tokens(
        &self,
        provider: &OracleProvider<Self::Api>,
    ) -> (EgldOrEsdtTokenIdentifier, EgldOrEsdtTokenIdentifier) {
        (
            provider.base_token_id.clone(),
            provider.quote_token_id.clone(),
        )
    }

    /// Converts a Normal or Derived token to an LP token
    fn convert_token_to_lp(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        base_token: &EgldOrEsdtTokenIdentifier,
        quote_token: &EgldOrEsdtTokenIdentifier,
        to_provider: &OracleProvider<Self::Api>,
        from_provider: &OracleProvider<Self::Api>,
        caller: &ManagedAddress,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        // Get initial reserves to estimate ratio
        let (r_base, r_quote, _) = self.get_reserves(&to_provider.oracle_contract_address);

        // Determine if input token is base or quote
        let is_from_base = from_token == base_token
            || (from_token.is_egld()
                && base_token.clone().unwrap_esdt().ticker() == self.get_wegld_token_id());

        // Set input token and other token based on which one the input token is
        let (input_token, other_token, r_input, r_other) = if is_from_base {
            (base_token, quote_token, r_base, r_quote)
        } else {
            (quote_token, base_token, r_quote, r_base)
        };

        // Calculate swap amount based on the current reserves ratio
        // For optimal LP creation, we need to match the reserves ratio
        // Reserve ratio input:other determines what portion to swap

        // Scale for precision (avoid division errors with small numbers)
        let scale = BigUint::from(BPS);

        // Calculate what percentage of our input should be swapped to maintain ratio
        // ratio_factor = r_other / (r_input + r_other)
        let ratio_numerator = r_other.clone() * &scale;
        let ratio_denominator = r_input.clone() + r_other.clone();
        let ratio_factor = ratio_numerator / ratio_denominator;

        // Calculate swap amount
        let swap_amount = from_amount.clone() * &ratio_factor / &scale;

        // Ensure we're swapping a positive amount
        require!(swap_amount > 0, ERROR_ZERO_AMOUNT);

        // Swap tokens via aggregator to get the other token
        let other_token_payment = self.convert_token_from_to(
            &self.token_oracle(other_token).get(),
            other_token,
            from_token,
            &swap_amount,
            from_provider,
            caller,
            steps.clone(),
            limits.clone(),
        );

        // Calculate remaining amount of input token after swap
        let remaining_input_amount = from_amount.clone() - &swap_amount;
        let received_other_amount = other_token_payment.amount;

        // Handle EGLD to WEGLD wrapping if needed for LP creation
        let final_input_amount = if from_token.is_egld()
            && input_token.clone().unwrap_esdt().ticker() == self.get_wegld_token_id()
        {
            self.wrap_egld(&remaining_input_amount);
            remaining_input_amount
        } else {
            remaining_input_amount
        };

        // Prepare amounts for LP token creation, ensuring correct order (base, quote)
        let (base_amount, quote_amount) = if is_from_base {
            (final_input_amount, received_other_amount)
        } else {
            (received_other_amount, final_input_amount)
        };

        // Create LP token with the amounts we have
        let lp_received = self.create_lp_token(
            &to_provider.oracle_contract_address,
            base_token,
            quote_token,
            base_amount,
            quote_amount,
            to_token,
        );

        require!(lp_received.amount > 0, ERROR_ZERO_AMOUNT);
        require!(lp_received.token_identifier == *to_token, ERROR_WRONG_TOKEN);

        lp_received
    }

    fn convert_token_from_to(
        &self,
        to_provider: &OracleProvider<Self::Api>,
        to_token: &EgldOrEsdtTokenIdentifier,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        from_provider: &OracleProvider<Self::Api>,
        caller: &ManagedAddress,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        if to_token == from_token {
            return EgldOrEsdtTokenPayment::new(to_token.clone(), 0, from_amount.clone());
        }
        match (
            from_provider.oracle_type.clone(),
            to_provider.oracle_type.clone(),
        ) {
            (OracleType::Normal, OracleType::Derived) => {
                if from_token == &to_provider.base_token_id {
                    self.convert_to_lsd(from_token, &from_amount, to_provider)
                } else {
                    self.swap_tokens(to_token, from_token, from_amount, caller, steps, limits)
                }
            },
            // Normal to LP: Convert if part of LP, handling EGLD/WEGLD
            (OracleType::Normal, OracleType::Lp) | (OracleType::Derived, OracleType::Lp) => {
                let (base_token, quote_token) = self.get_lp_tokens(to_provider);
                if self.is_token_in_lp(from_token, &base_token, &quote_token) {
                    self.convert_token_to_lp(
                        from_token,
                        from_amount,
                        to_token,
                        &base_token,
                        &quote_token,
                        to_provider,
                        from_provider,
                        caller,
                        steps,
                        limits,
                    )
                } else {
                    sc_panic!(
                        "Token: {} is not part of the LP token: {}",
                        from_token,
                        to_token
                    );
                }
            },
            // LP to Normal/Derived: Unified conversion
            (OracleType::Lp, OracleType::Normal) | (OracleType::Lp, OracleType::Derived) => {
                let (base_token, quote_token) = self.get_lp_tokens(from_provider);
                if self.is_token_in_lp(to_token, &base_token, &quote_token) {
                    self.convert_lp_to_token(
                        from_token,
                        from_amount,
                        to_token,
                        &base_token,
                        &quote_token,
                        to_provider,
                        from_provider,
                        caller,
                        steps,
                        limits,
                    )
                } else {
                    sc_panic!(
                        "Target token: {} is not part of the LP token: {}",
                        to_token,
                        from_token
                    );
                }
            },
            // Other cases (simplified for brevity)
            _ => self.swap_tokens(to_token, from_token, from_amount, caller, steps, limits),
        }
    }

    /// Checks if a token is part of the LP, considering EGLD/WEGLD
    fn is_token_in_lp(
        &self,
        token: &EgldOrEsdtTokenIdentifier,
        base_token: &EgldOrEsdtTokenIdentifier,
        quote_token: &EgldOrEsdtTokenIdentifier,
    ) -> bool {
        let wegld = self.get_wegld_token_id();
        token == base_token
            || token == quote_token
            || (token.is_egld() && base_token.clone().unwrap_esdt().ticker() == wegld)
            || (token.is_egld() && quote_token.clone().unwrap_esdt().ticker() == wegld)
    }

    /// Converts an LP token to a Normal or Derived token
    fn convert_lp_to_token(
        &self,
        from_token: &EgldOrEsdtTokenIdentifier,
        from_amount: &BigUint,
        to_token: &EgldOrEsdtTokenIdentifier,
        base_token: &EgldOrEsdtTokenIdentifier,
        quote_token: &EgldOrEsdtTokenIdentifier,
        to_provider: &OracleProvider<Self::Api>,
        from_provider: &OracleProvider<Self::Api>,
        caller: &ManagedAddress,
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        // Split the LP token into its constituent base and quote tokens
        let (base_payment, quote_payment) = self
            .split_collateral(
                from_amount,
                &from_token.as_esdt_option().unwrap(),
                &from_provider.oracle_contract_address,
            )
            .into_tuple();

        // Determine if the target token is the base or quote token
        let is_target_base = to_token == base_token
            || (to_token.is_egld()
                && base_token.clone().unwrap_esdt().ticker() == self.get_wegld_token_id());

        // Assign target tokens and other tokens based on which one is the desired output
        let (target_payment, other_payment, other_token) = if is_target_base {
            (base_payment, quote_payment, quote_token)
        } else {
            (quote_payment, base_payment, base_token)
        };

        // Handle WEGLD to EGLD conversion if needed for the other token
        let other_token_identifier =
            if other_payment.token_identifier.ticker() == self.get_wegld_token_id() {
                self.unwrap_wegld(&other_payment.amount, &other_payment.token_identifier);
                EgldOrEsdtTokenIdentifier::egld()
            } else {
                other_token.clone()
            };

        // Convert the other token to the target token
        let converted_payment = self.convert_token_from_to(
            to_provider,
            to_token,
            &other_token_identifier,
            &other_payment.amount,
            &self.token_oracle(other_token).get(),
            caller,
            steps,
            limits,
        );

        // Combine all received amounts of the target token
        let total_amount = target_payment.amount + converted_payment.amount;

        // Handle WEGLD to EGLD unwrapping if needed for the final result
        let final_amount = if to_token.is_egld()
            && target_payment.token_identifier.ticker() == self.get_wegld_token_id()
        {
            self.unwrap_wegld(&total_amount, &target_payment.token_identifier);
            total_amount
        } else {
            total_amount
        };

        // Create the final payment object
        let result = EgldOrEsdtTokenPayment::new(to_token.clone(), 0, final_amount);

        // Validate the result
        require!(result.amount > 0, ERROR_ZERO_AMOUNT);
        require!(result.token_identifier == *to_token, ERROR_WRONG_TOKEN);

        result
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
                .typed(proxy_xegld::LiquidStakingProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else if oracle_collateral.exchange_source == ExchangeSource::LXOXNO {
            self.tx()
                .to(&oracle_collateral.oracle_contract_address)
                .typed(proxy_lxoxno::RsLiquidXoxnoProxy)
                .get_exchange_rate()
                .returns(ReturnsResult)
                .sync_call_readonly()
        } else {
            sc_panic!("Strategy is not possible due to LSD conversion not existing!");
        }
    }

    fn swap_tokens(
        self,
        to: &EgldOrEsdtTokenIdentifier,
        from: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        caller: &ManagedAddress,
        steps_opt: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits_opt: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        // Ensure steps and limits are provided
        require!(
            steps_opt.is_some() && limits_opt.is_some(),
            "Steps and limits are required"
        );

        let steps = steps_opt.unwrap();
        let limits = limits_opt.unwrap();
        // Set up the aggregator contract call
        let call = self
            .tx()
            .to(self.aggregator().get())
            .typed(AggregatorContractProxy);

        // Collect all received payments in a unified format
        let received_payments = if from.is_esdt() {
            let second_call = call
                .aggregate_esdt(
                    steps,
                    limits,
                    to.is_egld(),
                    OptionalValue::<ManagedAddress>::None,
                )
                .egld_or_single_esdt(from, 0, amount);

            let back_transfers = second_call.returns(ReturnsBackTransfersReset).sync_call();
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

            payments
        } else {
            let back_transfers = call
                .aggregate_egld(steps, limits, OptionalValue::<ManagedAddress>::None)
                .egld(amount)
                .returns(ReturnsBackTransfersReset)
                .sync_call();

            require!(back_transfers.total_egld_amount == 0, ERROR_ZERO_AMOUNT);

            back_transfers
                .esdt_payments
                .into_iter()
                .map(|esdt| {
                    EgldOrEsdtTokenPayment::new(
                        EgldOrEsdtTokenIdentifier::esdt(esdt.token_identifier),
                        esdt.token_nonce,
                        esdt.amount,
                    )
                })
                .collect()
        };

        // Process payments to extract desired token and refunds
        let mut wanted_result = EgldOrEsdtTokenPayment::new(to.clone(), 0, BigUint::from(0u32));
        let mut refunds = ManagedVec::new();

        for payment in received_payments.iter() {
            if payment.token_identifier == *to {
                wanted_result.amount += &payment.amount;
            } else {
                refunds.push(payment.clone());
            }
        }

        // Send any refunds to the caller
        if refunds.len() > 0 {
            self.tx()
                .to(caller)
                .payment(refunds)
                .transfer_if_not_empty();
        }

        // Ensure we received the desired token
        require!(
            wanted_result.amount > 0,
            "No tokens received from aggregator"
        );

        wanted_result
    }

    fn get_reserves(&self, oracle_address: &ManagedAddress) -> (BigUint, BigUint, BigUint) {
        self.tx()
            .to(oracle_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .get_reserves_and_total_supply()
            .returns(ReturnsResult)
            .sync_call_readonly()
            .into_tuple()
    }

    fn estimate(
        &self,
        oracle_address: &ManagedAddress,
        token_in: &TokenIdentifier,
        amount: &BigUint,
    ) -> BigUint {
        self.tx()
            .to(oracle_address)
            .typed(proxy_xexchange_pair::PairProxy)
            .get_equivalent(token_in, amount)
            .returns(ReturnsResult)
            .sync_call_readonly()
    }

    fn wrap_egld(&self, amount: &BigUint) {
        self.tx()
            .to(self.wegld_wrapper().get())
            .typed(proxy_wegld::EgldEsdtSwapProxy)
            .wrap_egld()
            .egld(amount)
            .sync_call();
    }

    fn unwrap_wegld(&self, amount: &BigUint, token: &TokenIdentifier) {
        self.tx()
            .to(self.wegld_wrapper().get())
            .typed(proxy_wegld::EgldEsdtSwapProxy)
            .unwrap_egld()
            .single_esdt(token, 0, amount)
            .sync_call();
    }
}
