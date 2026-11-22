//! Version-pinned, deterministic algorithm interface and isolated state registry.
//! This crate deliberately has no database, payment, identity or execution authority.

use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};
use std::collections::{BTreeMap,BTreeSet};
use thiserror::Error;

#[derive(Debug,Error,PartialEq)]
pub enum RuntimeError {
 #[error("invalid algorithm manifest: {0}")] InvalidManifest(String),
 #[error("algorithm version is not registered")] UnknownVersion,
 #[error("required feature is missing: {0}")] MissingFeature(String),
 #[error("configuration is invalid: {0}")] InvalidConfig(String),
}

#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq)]
pub struct AlgorithmManifest {
 pub algorithm_id:String,pub display_name:String,pub version:String,pub runtime_contract_version:u16,
 pub supported_asset_classes:Vec<String>,pub supported_instruments:Vec<String>,pub supported_timeframes:Vec<String>,
 pub required_features:Vec<String>,pub optional_features:Vec<String>,pub status:AlgorithmStatus,
}
#[derive(Debug,Clone,Copy,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="snake_case")]
pub enum AlgorithmStatus { Preview,Active,Deprecated,Disabled }
impl AlgorithmManifest {
 pub fn validate(&self)->Result<(),RuntimeError>{
  if self.algorithm_id.trim().is_empty()||self.display_name.trim().is_empty()||self.version.trim().is_empty()||self.runtime_contract_version==0{return Err(RuntimeError::InvalidManifest("identity, semantic version and contract version are required".into()));}
  for values in [&self.supported_asset_classes,&self.supported_timeframes,&self.required_features] {if values.is_empty(){return Err(RuntimeError::InvalidManifest("asset classes, timeframes and required features must be declared".into()));}}
  if self.required_features.iter().any(|f|self.optional_features.contains(f)){return Err(RuntimeError::InvalidManifest("a feature cannot be both required and optional".into()));}
  Ok(())
 }
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct AlgorithmInput {
 pub instrument_id:String,pub asset_class:String,pub timeframe:String,pub ts_ms:i64,
 pub features:BTreeMap<String,f64>,pub source_event_ids:Vec<String>,pub config:BTreeMap<String,f64>,
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct AlgorithmOutput {
 pub algorithm_id:String,pub algorithm_version:String,pub config_fingerprint:String,pub ready:bool,
 pub action:String,pub score:Option<f64>,pub risk_diagnostics:BTreeMap<String,f64>,pub rationale:String,
 pub source_event_ids:Vec<String>,
}
/// Implementations must be pure: no IO, wall-clock reads, mutable globals or hidden user state.
pub trait ResearchAlgorithm: Send+Sync {
 fn manifest(&self)->&AlgorithmManifest;
 fn evaluate(&self,input:&AlgorithmInput)->Result<AlgorithmOutput,RuntimeError>;
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq,Eq,PartialOrd,Ord)]
pub struct StateNamespace {pub algorithm_id:String,pub version:String,pub instrument_id:String,pub venue:String,pub timeframe:String}
#[derive(Debug,Clone,Default)]
pub struct IsolatedStateStore {states:BTreeMap<StateNamespace,BTreeMap<String,String>>}
impl IsolatedStateStore {
 pub fn put(&mut self,key:StateNamespace,name:String,value:String){self.states.entry(key).or_default().insert(name,value);}
 pub fn get(&self,key:&StateNamespace,name:&str)->Option<&str>{self.states.get(key)?.get(name).map(String::as_str)}
 pub fn namespace_count(&self)->usize{self.states.len()}
}
#[derive(Default)]
pub struct AlgorithmRegistry {algorithms:BTreeMap<(String,String),Box<dyn ResearchAlgorithm>>}
impl AlgorithmRegistry {
 pub fn register(&mut self,algorithm:Box<dyn ResearchAlgorithm>)->Result<(),RuntimeError>{
  let m=algorithm.manifest();m.validate()?;let key=(m.algorithm_id.clone(),m.version.clone());
  if self.algorithms.contains_key(&key){return Err(RuntimeError::InvalidManifest("duplicate algorithm ID/version".into()));}
  self.algorithms.insert(key,algorithm);Ok(())
 }
 pub fn evaluate(&self,id:&str,version:&str,input:&AlgorithmInput)->Result<AlgorithmOutput,RuntimeError>{
  let algorithm=self.algorithms.get(&(id.into(),version.into())).ok_or(RuntimeError::UnknownVersion)?;
  let manifest=algorithm.manifest();
  if manifest.status!=AlgorithmStatus::Active{return Err(RuntimeError::InvalidManifest("algorithm is not active".into()));}
  if !manifest.supported_asset_classes.contains(&input.asset_class)||!manifest.supported_timeframes.contains(&input.timeframe)
   ||(!manifest.supported_instruments.is_empty()&&!manifest.supported_instruments.contains(&input.instrument_id)){return Err(RuntimeError::InvalidManifest("instrument, asset class or timeframe is unsupported".into()));}
  for feature in &manifest.required_features {if !input.features.get(feature).is_some_and(|v|v.is_finite()){return Err(RuntimeError::MissingFeature(feature.clone()));}}
  let bytes=serde_json::to_vec(&input.config).map_err(|e|RuntimeError::InvalidConfig(e.to_string()))?;
  let fingerprint=format!("{:x}",Sha256::digest(bytes));
  let mut output=algorithm.evaluate(input)?;
  if output.algorithm_id!=id||output.algorithm_version!=version{return Err(RuntimeError::InvalidManifest("algorithm output identity does not match registered version".into()));}
  output.config_fingerprint=fingerprint;output.source_event_ids=input.source_event_ids.clone();Ok(output)
 }
 pub fn versions(&self)->Vec<(String,String)>{self.algorithms.keys().cloned().collect()}
}
#[derive(Debug,Clone,Serialize,Deserialize,PartialEq)]
pub struct ComparisonKey {pub dataset_fingerprint:String,pub instrument_id:String,pub timeframe:String,pub from_ts_ms:i64,pub to_ts_ms:i64,pub fee_bps:f64,pub slippage_bps:f64}
impl ComparisonKey {
 pub fn validate(&self)->Result<(),RuntimeError>{
  if self.dataset_fingerprint.is_empty()||self.instrument_id.is_empty()||self.from_ts_ms<=0||self.to_ts_ms<=self.from_ts_ms
   ||!self.fee_bps.is_finite()||self.fee_bps<0.0||!self.slippage_bps.is_finite()||self.slippage_bps<0.0{return Err(RuntimeError::InvalidConfig("comparison dataset, window and execution assumptions must be identical and valid".into()));}Ok(())
 }
}
pub fn compatible_comparison(keys:&[ComparisonKey])->bool{
 if keys.is_empty()||keys.iter().any(|k|k.validate().is_err()){return false;}
 keys.iter().all(|k|k==&keys[0])
}
pub fn required_feature_set(manifests:&[AlgorithmManifest])->BTreeSet<String>{
 manifests.iter().flat_map(|m|m.required_features.iter().cloned()).collect()
}

#[cfg(test)]
mod tests{
 use super::*;
 struct Stub{manifest:AlgorithmManifest,action:String}
 impl ResearchAlgorithm for Stub{
  fn manifest(&self)->&AlgorithmManifest{&self.manifest}
  fn evaluate(&self,input:&AlgorithmInput)->Result<AlgorithmOutput,RuntimeError>{Ok(AlgorithmOutput{algorithm_id:self.manifest.algorithm_id.clone(),algorithm_version:self.manifest.version.clone(),config_fingerprint:String::new(),ready:true,action:self.action.clone(),score:Some(*input.features.values().next().unwrap()),risk_diagnostics:BTreeMap::new(),rationale:"deterministic test algorithm".into(),source_event_ids:vec![]})}
 }
 fn stub(id:&str,version:&str,action:&str)->Box<dyn ResearchAlgorithm>{Box::new(Stub{manifest:AlgorithmManifest{algorithm_id:id.into(),display_name:id.into(),version:version.into(),runtime_contract_version:1,supported_asset_classes:vec!["crypto".into()],supported_instruments:vec![],supported_timeframes:vec!["5m".into()],required_features:vec!["flow".into()],optional_features:vec![],status:AlgorithmStatus::Active},action:action.into()})}
 fn input()->AlgorithmInput{AlgorithmInput{instrument_id:"BTCUSDT".into(),asset_class:"crypto".into(),timeframe:"5m".into(),ts_ms:10,features:BTreeMap::from([("flow".into(),0.5)]),source_event_ids:vec!["e1".into()],config:BTreeMap::from([("threshold".into(),0.2)])}}
 #[test]fn registry_pins_distinct_algorithm_versions(){let mut r=AlgorithmRegistry::default();r.register(stub("flow","1.0.0","BUY")).unwrap();r.register(stub("flow","2.0.0","WAIT")).unwrap();assert_eq!(r.evaluate("flow","1.0.0",&input()).unwrap().action,"BUY");assert_eq!(r.evaluate("flow","2.0.0",&input()).unwrap().action,"WAIT");}
 #[test]fn required_features_fail_closed(){let mut r=AlgorithmRegistry::default();r.register(stub("flow","1.0.0","BUY")).unwrap();let mut i=input();i.features.clear();assert!(matches!(r.evaluate("flow","1.0.0",&i),Err(RuntimeError::MissingFeature(_))));}
 #[test]fn mutable_state_is_isolated_by_algorithm_version_and_instrument(){let mut s=IsolatedStateStore::default();let a=StateNamespace{algorithm_id:"a".into(),version:"1".into(),instrument_id:"BTC".into(),venue:"x".into(),timeframe:"5m".into()};let mut b=a.clone();b.algorithm_id="b".into();s.put(a.clone(),"window".into(),"a-state".into());assert_eq!(s.get(&a,"window"),Some("a-state"));assert_eq!(s.get(&b,"window"),None);}
 #[test]fn comparisons_require_identical_data_and_cost_assumptions(){let k=ComparisonKey{dataset_fingerprint:"data-v1".into(),instrument_id:"BTC".into(),timeframe:"5m".into(),from_ts_ms:1,to_ts_ms:10,fee_bps:2.0,slippage_bps:1.0};assert!(compatible_comparison(&[k.clone(),k.clone()]));let mut other=k.clone();other.fee_bps=3.0;assert!(!compatible_comparison(&[k,other]));}
 #[test]fn duplicate_algorithm_versions_are_rejected(){let mut r=AlgorithmRegistry::default();r.register(stub("flow","1.0.0","BUY")).unwrap();assert!(r.register(stub("flow","1.0.0","SELL")).is_err());}
}
