use common_constants::WEGLD_TICKER;
use common_structs::{ExchangeSource, OracleProvider, OracleType};

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
        // sc_panic!(
        //     "Creating LP token: {}, {}, {}, {}",
        //     first_token,
        //     second_token,
        //     first_token_amount,
        //     secont_token_amount
        // );
        payments.push(EsdtTokenPayment::new(
            second_token.clone().into_esdt_option().unwrap(),
            0,
            secont_token_amount,
        ));
        payments.push(EsdtTokenPayment::new(
            first_token.clone().into_esdt_option().unwrap(),
            0,
            first_token_amount,
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
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        let (r_base, r_quote, _) = self.get_reserves(&to_provider.oracle_contract_address);
        let is_from_base = from_token == base_token
            || (from_token.is_egld()
                && base_token.clone().unwrap_esdt().ticker() == self.get_wegld_token_id());
        let (token_a, token_b, r_a, r_b) = if is_from_base {
            (base_token, quote_token, r_base, r_quote)
        } else {
            (quote_token, base_token, r_quote, r_base)
        };

        // Calculate amount to swap based on instant reserve ratio
        let s = (from_amount * &r_b) / (r_a + &r_b);
        // sc_panic!(
        //     "Converting Token: {} to LP: {}, token_a: {}, token_b: {}, s: {}",
        //     from_token,
        //     to_token,
        //     token_a,
        //     token_b,
        //     s,
        // );

        let y = self.convert_token_from_to(
            &self.token_oracle(token_b).get(),
            token_b,
            from_token,
            &s,
            from_provider,
            steps.clone(),
            limits.clone(),
        );

        // Handle EGLD to WEGLD wrapping
        let amount_a = from_amount - &s;
        let amount_b = y.amount;
        if from_token.is_egld() {
            self.wrap_egld(&amount_a);
        }

        // Create LP token
        let lp_amount = self.create_lp_token(
            &to_provider.oracle_contract_address,
            token_a,
            token_b,
            amount_a,
            amount_b,
        );

        EgldOrEsdtTokenPayment::new(lp_amount.0, 0, lp_amount.1)
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
            }
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
            }
            // Other cases (simplified for brevity)
            _ => self.swap_tokens(to_token, from_token, from_amount, steps, limits),
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
        steps: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        let (amount_base, amount_quote) = self
            .split_collateral(
                from_amount,
                &from_token.as_esdt_option().unwrap(),
                &from_provider.oracle_contract_address,
            )
            .into_tuple();

        let (target_amount, other_amount, other_token) = if to_token == base_token
            || (to_token.is_egld()
                && base_token.clone().unwrap_esdt().ticker() == self.get_wegld_token_id())
        {
            (amount_base, amount_quote, quote_token)
        } else {
            (amount_quote, amount_base, base_token)
        };

        let final_other = if other_amount.token_identifier.ticker() == self.get_wegld_token_id() {
            self.unwrap_wegld(&other_amount.amount, &other_amount.token_identifier);

            EgldOrEsdtTokenIdentifier::egld()
        } else {
            other_token.clone()
        };

        let converted = self.convert_token_from_to(
            to_provider,
            to_token,
            &final_other,
            &other_amount.amount,
            &self.token_oracle(other_token).get(),
            steps,
            limits,
        );

        // Handle WEGLD to EGLD unwrapping
        let final_amount = target_amount.amount + converted.amount;
        if to_token.is_egld()
            && target_amount.token_identifier.ticker() == self.get_wegld_token_id()
        {
            self.unwrap_wegld(&final_amount, &target_amount.token_identifier);
        }

        EgldOrEsdtTokenPayment::new(to_token.clone(), 0, final_amount)
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

    fn swap_tokens(
        self,
        to: &EgldOrEsdtTokenIdentifier,
        from: &EgldOrEsdtTokenIdentifier,
        amount: &BigUint,
        steps_opt: Option<ManagedVec<AggregatorStep<Self::Api>>>,
        limits_opt: Option<ManagedVec<TokenAmount<Self::Api>>>,
    ) -> EgldOrEsdtTokenPayment {
        require!(
            steps_opt.is_some() && limits_opt.is_some(),
            "Steps and limits are required"
        );
        let mut steps = steps_opt.unwrap();
        let limits = limits_opt.unwrap();
        let call = self
            .tx()
            .to(self.aggregator().get())
            .typed(AggregatorContractProxy);
        let mut first_step = steps.get(0).clone();
        first_step.amount_in = amount.clone();
        let _ = steps.set(0, first_step);

        if from.is_esdt() {
            let second_call = call
                .aggregate_esdt(
                    steps,
                    limits,
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
                .aggregate_egld(steps, limits, OptionalValue::<ManagedAddress>::None)
                .egld(amount)
                .returns(ReturnsBackTransfersSingleESDT)
                .sync_call();

            result.into_multi_egld_or_esdt_payment()
        }
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
