//! Deterministic actuarial and insurance research primitives. Not underwriting advice.
use serde::{Deserialize,Serialize};
use thiserror::Error;

#[derive(Debug,Error,PartialEq)]
pub enum InsuranceError{
 #[error("inputs must be finite")] NonFinite,
 #[error("frequency, severity and exposure must be non-negative; exposure must be positive")] InvalidExposure,
 #[error("probability must be between zero and one")] InvalidProbability,
 #[error("expense and profit loadings must be non-negative")] InvalidLoading,
 #[error("retention and limit must be valid")] InvalidTreaty,
}
fn finite(x:&[f64])->bool{x.iter().all(|v|v.is_finite())}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct LossModel{pub claim_frequency_per_exposure:f64,pub mean_severity:f64,pub exposure_units:f64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct PurePremium{pub expected_aggregate_loss:f64,pub expected_claim_count:f64,pub pure_premium_per_exposure:f64}
pub fn pure_premium(m:LossModel)->Result<PurePremium,InsuranceError>{
 let x=[m.claim_frequency_per_exposure,m.mean_severity,m.exposure_units];
 if !finite(&x){return Err(InsuranceError::NonFinite)}
 if m.claim_frequency_per_exposure<0.0||m.mean_severity<0.0||m.exposure_units<=0.0{return Err(InsuranceError::InvalidExposure)}
 let count=m.claim_frequency_per_exposure*m.exposure_units;
 let loss=count*m.mean_severity;
 Ok(PurePremium{expected_aggregate_loss:loss,expected_claim_count:count,pure_premium_per_exposure:loss/m.exposure_units})
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct PremiumLoadings{pub expense_ratio:f64,pub commission_ratio:f64,pub risk_margin_ratio:f64,pub profit_margin_ratio:f64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct GrossPremium{pub pure_premium:f64,pub indicated_premium:f64,pub loading_amount:f64}
pub fn gross_premium(pure:f64,l:PremiumLoadings)->Result<GrossPremium,InsuranceError>{
 let x=[pure,l.expense_ratio,l.commission_ratio,l.risk_margin_ratio,l.profit_margin_ratio];
 if !finite(&x){return Err(InsuranceError::NonFinite)}
 if pure<0.0||[l.expense_ratio,l.commission_ratio,l.risk_margin_ratio,l.profit_margin_ratio].iter().any(|v|*v<0.0)||l.expense_ratio+l.commission_ratio+l.risk_margin_ratio+l.profit_margin_ratio>=1.0{return Err(InsuranceError::InvalidLoading)}
 let p=pure/(1.0-l.expense_ratio-l.commission_ratio-l.risk_margin_ratio-l.profit_margin_ratio);
 Ok(GrossPremium{pure_premium:pure,indicated_premium:p,loading_amount:p-pure})
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct FrequencySeverity{pub annual_frequency:f64,pub mean_severity:f64,pub severity_std_dev:f64,pub exposure_units:f64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct AggregateLossEstimate{pub expected_loss:f64,pub variance:f64,pub standard_deviation:f64}
pub fn compound_poisson(m:FrequencySeverity)->Result<AggregateLossEstimate,InsuranceError>{
 let x=[m.annual_frequency,m.mean_severity,m.severity_std_dev,m.exposure_units];
 if !finite(&x){return Err(InsuranceError::NonFinite)}
 if m.annual_frequency<0.0||m.mean_severity<0.0||m.severity_std_dev<0.0||m.exposure_units<=0.0{return Err(InsuranceError::InvalidExposure)}
 let lambda=m.annual_frequency*m.exposure_units;
 let second=m.mean_severity*m.mean_severity+m.severity_std_dev*m.severity_std_dev;
 let variance=lambda*second;
 Ok(AggregateLossEstimate{expected_loss:lambda*m.mean_severity,variance,standard_deviation:variance.sqrt()})
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct ReinsuranceTreaty{pub per_claim_retention:f64,pub per_claim_limit:f64,pub quota_share_ceded:f64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct ReinsuranceRecovery{pub gross_claim:f64,pub ceded_recovery:f64,pub net_claim:f64}
pub fn apply_reinsurance(claim:f64,t:ReinsuranceTreaty)->Result<ReinsuranceRecovery,InsuranceError>{
 if !finite(&[claim,t.per_claim_retention,t.per_claim_limit,t.quota_share_ceded]){return Err(InsuranceError::NonFinite)}
 if claim<0.0||t.per_claim_retention<0.0||t.per_claim_limit<0.0||t.per_claim_limit>0.0&&t.per_claim_limit<t.per_claim_retention{return Err(InsuranceError::InvalidTreaty)}
 if !(0.0..=1.0).contains(&t.quota_share_ceded){return Err(InsuranceError::InvalidProbability)}
 // Per-risk excess-of-loss layer, then quota share applies to the retained net amount.
 let layer=(claim-t.per_claim_retention).max(0.0);
 let excess_recovery=if t.per_claim_limit==0.0{layer}else{layer.min(t.per_claim_limit-t.per_claim_retention)};
 let after_xol=claim-excess_recovery;
 let quota_recovery=after_xol*t.quota_share_ceded;
 let recovery=excess_recovery+quota_recovery;
 Ok(ReinsuranceRecovery{gross_claim:claim,ceded_recovery:recovery,net_claim:claim-recovery})
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct ReserveInput{pub reported_case_reserve:f64,pub ibnr_expected:f64,pub expected_future_expense:f64,pub discount_rate:f64,pub duration_years:f64}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
pub struct ReserveEstimate{pub undiscounted_reserve:f64,pub discounted_reserve:f64}
pub fn reserve_estimate(i:ReserveInput)->Result<ReserveEstimate,InsuranceError>{
 let x=[i.reported_case_reserve,i.ibnr_expected,i.expected_future_expense,i.discount_rate,i.duration_years];
 if !finite(&x){return Err(InsuranceError::NonFinite)}
 if i.reported_case_reserve<0.0||i.ibnr_expected<0.0||i.expected_future_expense<0.0||i.duration_years<0.0||i.discount_rate<=-1.0{return Err(InsuranceError::InvalidExposure)}
 let total=i.reported_case_reserve+i.ibnr_expected+i.expected_future_expense;
 Ok(ReserveEstimate{undiscounted_reserve:total,discounted_reserve:total/(1.0+i.discount_rate).powf(i.duration_years)})
}

#[cfg(test)] mod tests{
 use super::*;
 #[test] fn expected_loss_and_loaded_premium(){let p=pure_premium(LossModel{claim_frequency_per_exposure:0.1,mean_severity:5000.0,exposure_units:100.0}).unwrap();assert!((p.expected_aggregate_loss-50000.0).abs()<1e-9);assert!((p.pure_premium_per_exposure-500.0).abs()<1e-9);let g=gross_premium(500.0,PremiumLoadings{expense_ratio:0.1,commission_ratio:0.05,risk_margin_ratio:0.05,profit_margin_ratio:0.05}).unwrap();assert!((g.indicated_premium-666.6666667).abs()<1e-5);}
 #[test] fn compound_poisson_variance(){let a=compound_poisson(FrequencySeverity{annual_frequency:2.0,mean_severity:100.0,severity_std_dev:50.0,exposure_units:10.0}).unwrap();assert!((a.expected_loss-2000.0).abs()<1e-9);assert!((a.variance-250000.0).abs()<1e-9);}
 #[test] fn excess_of_loss_then_quota_share(){let r=apply_reinsurance(1000.0,ReinsuranceTreaty{per_claim_retention:200.0,per_claim_limit:600.0,quota_share_ceded:0.1}).unwrap();assert!((r.ceded_recovery-440.0).abs()<1e-9);assert!((r.net_claim-560.0).abs()<1e-9);}
 #[test] fn reserve_components(){let r=reserve_estimate(ReserveInput{reported_case_reserve:100.0,ibnr_expected:50.0,expected_future_expense:10.0,discount_rate:0.05,duration_years:2.0}).unwrap();assert!((r.undiscounted_reserve-160.0).abs()<1e-9);assert!(r.discounted_reserve<160.0);}
}
