//! Aggregates calculated observations; unavailable dimensions do not count as zero evidence.
use serde::{Deserialize,Serialize};use std::collections::BTreeMap;use crate::features::FeaturePoint;
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]pub struct PairInteraction{pub first:String,pub second:String,pub coefficient:f64}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]pub struct ConfluenceConfig{pub weights:BTreeMap<String,f64>,pub interactions:Vec<PairInteraction>,pub minimum_coverage:f64,pub maximum_interaction:f64}
impl Default for ConfluenceConfig{fn default()->Self{let weights=[("ofi",0.12),("open_interest_change",0.07),("funding_pressure",0.05),("liquidity_stress",0.07),("volatility",0.05),("fibonacci",0.09),("fvg",0.09),("value_area",0.09),("gex",0.07),("basis",0.03),("liquidation_pressure",0.10),("event_intensity",0.04),("price_momentum",0.13)].into_iter().map(|(k,v)|(k.into(),v)).collect();Self{weights,interactions:vec![PairInteraction{first:"ofi".into(),second:"open_interest_change".into(),coefficient:0.08},PairInteraction{first:"fibonacci".into(),second:"fvg".into(),coefficient:0.08},PairInteraction{first:"value_area".into(),second:"ofi".into(),coefficient:0.06}],minimum_coverage:0.45,maximum_interaction:0.25}}}
#[derive(Debug,Clone,Serialize,PartialEq)]pub struct ConfluenceResult{pub pressure:f64,pub agreement:f64,pub contradiction:f64,pub coverage:f64,pub risk_level:f64,pub interaction_contribution:f64,pub available_features:usize,pub action:&'static str,pub explanation:String}
pub fn calculate_confluence(points:&[FeaturePoint],cfg:&ConfluenceConfig)->Result<ConfluenceResult,String>{if !cfg.minimum_coverage.is_finite()||!(0.0..=1.0).contains(&cfg.minimum_coverage)||!cfg.maximum_interaction.is_finite()||!(0.0..=1.0).contains(&cfg.maximum_interaction)||cfg.weights.values().any(|w|!w.is_finite()||*w<0.0)||cfg.interactions.iter().any(|i|!i.coefficient.is_finite()||i.coefficient.abs()>cfg.maximum_interaction){return Err("invalid confluence configuration".into())}let map:BTreeMap<&str,&FeaturePoint>=points.iter().filter(|p|p.available&&p.confidence.is_finite()&&p.confidence>0.0&&p.value.is_finite()).map(|p|(p.name.as_str(),p)).collect();let total=cfg.weights.values().sum::<f64>();if total<=f64::EPSILON{return Err("confluence weights must have positive total".into())}let available=cfg.weights.iter().filter_map(|(n,w)|map.get(n.as_str()).map(|p|w*p.confidence)).sum::<f64>();let coverage=(available/total).clamp(0.0,1.0);let(mut direction,mut pos,mut neg,mut risk)=(0.0,0.0,0.0,0.0);for(n,w)in &cfg.weights{if let Some(p)=map.get(n.as_str()){let ew=w*p.confidence;direction+=ew*p.value;if p.value>0.0{pos+=ew*p.value.abs()}else{neg+=ew*p.value.abs()}risk+=ew*p.risk}}let base=if available>0.0{direction/available}else{0.0};let sum=pos+neg;let agreement=if sum>0.0{pos.max(neg)/sum}else{0.0};let contradiction=if sum>0.0{2.0*pos.min(neg)/sum}else{0.0};let mut interaction=0.0;for i in &cfg.interactions{if let(Some(a),Some(b))=(map.get(i.first.as_str()),map.get(i.second.as_str())){let av=if a.value.abs()>f64::EPSILON{a.value}else if a.context.abs()>f64::EPSILON{a.context}else{a.risk};let bv=if b.value.abs()>f64::EPSILON{b.value}else if b.context.abs()>f64::EPSILON{b.context}else{b.risk};interaction+=i.coefficient*av*bv*a.confidence.min(b.confidence)}}let interaction=interaction.clamp(-cfg.maximum_interaction,cfg.maximum_interaction);let pressure=(base+interaction).clamp(-1.0,1.0);let risk_level=if available>0.0{(risk/available).clamp(0.0,1.0)}else{0.0};let action=if coverage<cfg.minimum_coverage||agreement<0.60||contradiction>0.45||risk_level>0.85{"wait"}else if pressure>=0.20{"buy"}else if pressure<=-0.20{"sell"}else{"wait"};let explanation=format!("coverage={coverage:.3}; agreement={agreement:.3}; contradiction={contradiction:.3}; risk={risk_level:.3}; pressure={pressure:.3}");Ok(ConfluenceResult{pressure,agreement,contradiction,coverage,risk_level,interaction_contribution:interaction,available_features:map.len(),action,explanation})}
#[cfg(test)]mod tests{use super::*;fn p(n:&str,v:f64)->FeaturePoint{FeaturePoint::new(n,v,1.0)}#[test]fn confluence_reinforces_aligned_dimensions(){let x=[p("ofi",0.9),p("fibonacci",0.8),p("fvg",0.8),p("value_area",0.7),p("gex",0.6)];let r=calculate_confluence(&x,&ConfluenceConfig::default()).unwrap();assert!(r.pressure>0.0&&r.agreement>0.9)}#[test]fn contradictory_dimensions_wait(){let x=[p("ofi",1.0),p("fibonacci",-1.0),p("fvg",1.0),p("value_area",-1.0),p("gex",1.0)];assert_eq!(calculate_confluence(&x,&ConfluenceConfig::default()).unwrap().action,"wait")}#[test]fn missing_data_reduces_coverage(){let r=calculate_confluence(&[p("ofi",1.0)],&ConfluenceConfig::default()).unwrap();assert!(r.coverage<0.45);assert_eq!(r.action,"wait")}}
#[cfg(test)]
mod context_interaction_tests {
 use super::*;
 use crate::features::FeaturePoint;
 #[test]
 fn oi_and_gex_are_contextual_not_standalone_directional_votes() {
  let oi=FeaturePoint::with_context("open_interest_change",0.0,1.0,0.0,0.8);
  let gex=FeaturePoint::with_context("gex",0.0,1.0,0.7,-1.0);
  let ofi=FeaturePoint::new("ofi",0.9,1.0);
  let volatility=FeaturePoint::with_risk("volatility",0.0,1.0,0.8);
  assert_eq!(oi.value,0.0);
  assert_eq!(gex.value,0.0);
  let result=calculate_confluence(&[oi,gex,ofi,volatility],&ConfluenceConfig::default()).unwrap();
  assert!(result.interaction_contribution.is_finite());
  assert!(result.risk_level>0.0);
 }
}

#[cfg(test)]
mod regime_only_tests {
 use super::*;
 use crate::features::FeaturePoint;
 #[test]
 fn gamma_and_volatility_context_do_not_create_a_direction_alone() {
  let gex=FeaturePoint::with_context("gex",0.0,1.0,0.8,-1.0);
  let volatility=FeaturePoint::with_risk("volatility",0.0,1.0,0.8);
  let result=calculate_confluence(&[gex,volatility],&ConfluenceConfig::default()).unwrap();
  assert_eq!(result.pressure,0.0);
  assert_eq!(result.action,"wait");
  assert!(result.risk_level>0.0);
 }
}
