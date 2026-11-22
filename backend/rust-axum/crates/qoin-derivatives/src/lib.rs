//! Deterministic derivatives research primitives. No order routing or broker access.
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum DerivativesError {
    #[error("all numeric inputs must be finite")]
    NonFinite,
    #[error("spot, strike, volatility and time must be positive")]
    NonPositiveInput,
    #[error("rate, dividend yield and volatility must be in valid ranges")]
    InvalidParameter,
    #[error("option expiry must be positive")]
    Expired,
    #[error("contract multiplier must be positive")]
    InvalidMultiplier,
}

fn finite(xs: &[f64]) -> bool { xs.iter().all(|x| x.is_finite()) }
fn norm_pdf(x: f64) -> f64 { (-0.5*x*x).exp() / (2.0*std::f64::consts::PI).sqrt() }
fn norm_cdf(x: f64) -> f64 {
    // Abramowitz-Stegun 7.1.26; absolute error is below 8e-8.
    let z=x.abs(); let t=1.0/(1.0+0.2316419*z);
    let p=1.0-norm_pdf(z)*(0.319381530*t-0.356563782*t.powi(2)+1.781477937*t.powi(3)-1.821255978*t.powi(4)+1.330274429*t.powi(5));
    if x < 0.0 { 1.0-p } else { p }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="snake_case")]
pub enum OptionKind { Call, Put }

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct BlackScholesInput {
    pub kind: OptionKind, pub spot: f64, pub strike: f64, pub time_years: f64,
    pub risk_free_rate: f64, pub dividend_yield: f64, pub volatility: f64,
    pub multiplier: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct OptionMetrics {
    pub theoretical_price: f64, pub delta: f64, pub gamma: f64, pub vega_per_vol_point: f64,
    pub theta_per_day: f64, pub rho_per_rate_point: f64,
}

pub fn black_scholes(i: BlackScholesInput) -> Result<OptionMetrics, DerivativesError> {
    let x=[i.spot,i.strike,i.time_years,i.risk_free_rate,i.dividend_yield,i.volatility,i.multiplier];
    if !finite(&x) { return Err(DerivativesError::NonFinite); }
    if i.spot<=0.0 || i.strike<=0.0 || i.time_years<=0.0 || i.volatility<=0.0 { return Err(DerivativesError::NonPositiveInput); }
    if i.multiplier<=0.0 { return Err(DerivativesError::InvalidMultiplier); }
    if i.risk_free_rate<=-1.0 || i.dividend_yield<=-1.0 || i.volatility>10.0 { return Err(DerivativesError::InvalidParameter); }
    let s=i.spot; let k=i.strike; let t=i.time_years; let v=i.volatility; let r=i.risk_free_rate; let q=i.dividend_yield;
    let root=t.sqrt(); let d1=((s/k).ln()+(r-q+0.5*v*v)*t)/(v*root); let d2=d1-v*root;
    let disc_q=(-q*t).exp(); let disc_r=(-r*t).exp(); let pdf=norm_pdf(d1);
    let (price,delta,rho)=match i.kind {
        OptionKind::Call => (s*disc_q*norm_cdf(d1)-k*disc_r*norm_cdf(d2),disc_q*norm_cdf(d1),k*t*disc_r*norm_cdf(d2)/100.0),
        OptionKind::Put => (k*disc_r*norm_cdf(-d2)-s*disc_q*norm_cdf(-d1),disc_q*(norm_cdf(d1)-1.0),-k*t*disc_r*norm_cdf(-d2)/100.0),
    };
    let gamma=disc_q*pdf/(s*v*root);
    let vega=s*disc_q*pdf*root/100.0;
    let theta=match i.kind {
        OptionKind::Call => (-s*disc_q*pdf*v/(2.0*root)-r*k*disc_r*norm_cdf(d2)+q*s*disc_q*norm_cdf(d1))/365.0,
        OptionKind::Put => (-s*disc_q*pdf*v/(2.0*root)+r*k*disc_r*norm_cdf(-d2)-q*s*disc_q*norm_cdf(-d1))/365.0,
    };
    Ok(OptionMetrics { theoretical_price:price*i.multiplier, delta:delta*i.multiplier, gamma:gamma*i.multiplier, vega_per_vol_point:vega*i.multiplier, theta_per_day:theta*i.multiplier, rho_per_rate_point:rho*i.multiplier })
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FuturesFairValueInput { pub spot:f64, pub risk_free_rate:f64, pub dividend_yield:f64, pub time_years:f64, pub storage_cost_rate:f64 }
pub fn futures_fair_value(i:FuturesFairValueInput)->Result<f64,DerivativesError>{
    let x=[i.spot,i.risk_free_rate,i.dividend_yield,i.time_years,i.storage_cost_rate];
    if !finite(&x){return Err(DerivativesError::NonFinite)}
    if i.spot<=0.0||i.time_years<0.0{return Err(DerivativesError::NonPositiveInput)}
    Ok(i.spot*((i.risk_free_rate+i.storage_cost_rate-i.dividend_yield)*i.time_years).exp())
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ForwardInput { pub spot:f64,pub risk_free_rate:f64,pub dividend_yield:f64,pub time_years:f64 }
pub fn forward_value(i:ForwardInput)->Result<f64,DerivativesError>{
    futures_fair_value(FuturesFairValueInput{spot:i.spot,risk_free_rate:i.risk_free_rate,dividend_yield:i.dividend_yield,time_years:i.time_years,storage_cost_rate:0.0})
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SwapInput { pub notional:f64,pub fixed_rate:f64,pub discount_factors:[f64;16],pub accrual_fractions:[f64;16],pub periods:usize }
pub fn payer_swap_npv(i:SwapInput)->Result<f64,DerivativesError>{
    if i.periods==0||i.periods>16||!finite(&[i.notional,i.fixed_rate])||i.notional<=0.0||i.fixed_rate<0.0{return Err(DerivativesError::InvalidParameter)}
    let mut pv_fixed=0.0;let mut annuity=0.0;
    for n in 0..i.periods {let df=i.discount_factors[n];let a=i.accrual_fractions[n];if !df.is_finite()||!a.is_finite()||df<=0.0||df>1.5||a<=0.0||a>2.0{return Err(DerivativesError::InvalidParameter)} annuity+=df*a;pv_fixed+=df*a*i.fixed_rate;}
    // Floating leg is represented by par-reset identity: 1 - final discount factor.
    Ok(i.notional*((1.0-i.discount_factors[i.periods-1])-pv_fixed))
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn textbook_call_and_put_parity(){
  let base=BlackScholesInput{kind:OptionKind::Call,spot:100.0,strike:100.0,time_years:1.0,risk_free_rate:0.05,dividend_yield:0.0,volatility:0.2,multiplier:1.0};
  let c=black_scholes(base).unwrap(); assert!((c.theoretical_price-10.4506).abs()<0.002);
  let p=black_scholes(BlackScholesInput{kind:OptionKind::Put,..base}).unwrap();
  assert!((c.theoretical_price-p.theoretical_price-(100.0-100.0*(-0.05f64).exp())).abs()<0.002);
  assert!(c.gamma>0.0&&c.vega_per_vol_point>0.0&&c.theta_per_day<0.0);
 }
 #[test] fn rejects_bad_market_inputs(){let x=BlackScholesInput{kind:OptionKind::Call,spot:f64::NAN,strike:1.0,time_years:1.0,risk_free_rate:0.0,dividend_yield:0.0,volatility:0.2,multiplier:1.0};assert_eq!(black_scholes(x).unwrap_err(),DerivativesError::NonFinite);}
 #[test] fn carry_forward(){let f=forward_value(ForwardInput{spot:100.0,risk_free_rate:0.05,dividend_yield:0.02,time_years:1.0}).unwrap();assert!((f-103.04545).abs()<0.001);}
}
