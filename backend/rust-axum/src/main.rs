use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use mongodb::{
    bson::{doc, Document},
    options::{FindOneOptions, FindOptions, IndexOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, HashMap, HashSet, VecDeque}, env, sync::{Arc, RwLock}, time::Duration};
use futures_util::{SinkExt, StreamExt, TryStreamExt};
use tokio::sync::broadcast;
use tokio_tungstenite::connect_async;
use tower_http::trace::TraceLayer;

mod algorithm_catalogue;
mod algorithm_entitlements;
mod edge_strategy;
mod backtest;
mod vector_engine;
mod order_safety;
mod execution_lifecycle;
mod trading;
use edge_strategy::{DecisionInputEvent, EdgeDecision, EdgeStrategy, STRATEGY_KEY};
use qoin_vector_engine::{decide as decide_signal, decide_from_market_vector, EngineConfig, ForceObservation, Signal};
use qoin_market_space::{attribution::AttributionReport, hub::{HubError, NormalizationProfile, VariableHub}, FeatureAvailability, MarketFeature, MarketVector};
use qoin_set_theoretic_attention::{set_attention_weights, EvidenceSet, RelationshipEventType, RelationshipLedgerEvent, RelationshipRecord, VectorEvidenceConfig, Vocabulary};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
struct MarketEvent {
    event_id: String,
    venue: String,
    symbol: String,
    event_type: String,
    ts_ms: i64,
    #[serde(default)] price: Option<f64>,
    #[serde(default)] size: Option<f64>,
    #[serde(default)] side: Option<String>,
    #[serde(default)] bid: Option<f64>,
    #[serde(default)] ask: Option<f64>,
    #[serde(default)] bid_size: Option<f64>,
    #[serde(default)] ask_size: Option<f64>,
    #[serde(default)] funding_rate: Option<f64>,
    #[serde(default)] open_interest: Option<f64>,
    #[serde(default)] mark_price: Option<f64>,
    #[serde(default)] index_price: Option<f64>,
    #[serde(default)] liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
struct SymbolState {
    venue: String,
    symbol: String,
    event_count: usize,
    last_event_ts_ms: Option<i64>,
    last_price: Option<f64>,
    spread_bps: Option<f64>,
    signed_flow: f64,
    total_trade_size: f64,
    order_flow_imbalance: f64,
    event_intensity: f64,
    ewma_abs_return: f64,
    funding_rate: Option<f64>,
    open_interest: Option<f64>,
    mark_price: Option<f64>,
    index_price: Option<f64>,
    liquidation_events: usize,
    last_liquidation_side: Option<String>,
    state_quality: f64,
    last_v1_decision: Option<serde_json::Value>,
    last_v1_evaluated_at_ms: Option<i64>,
    order_book_depth: Option<f64>,
    open_interest_change: Option<f64>,
    #[serde(skip)] last_price_ts_ms: Option<i64>,
    #[serde(skip)] last_trade_ts_ms: Option<i64>,
    #[serde(skip)] last_book_ts_ms: Option<i64>,
    #[serde(skip)] last_funding_ts_ms: Option<i64>,
    #[serde(skip)] last_open_interest_ts_ms: Option<i64>,
    #[serde(skip)] last_basis_ts_ms: Option<i64>,
    #[serde(skip)] last_liquidation_ts_ms: Option<i64>,
    #[serde(skip)] recent_prices: VecDeque<f64>,
    #[serde(skip)] recent_returns: VecDeque<f64>,
    #[serde(skip)] recent_return_observations: VecDeque<(i64,f64)>,
    #[serde(skip)] recent_trade_flow: VecDeque<f64>,
    #[serde(skip)] recent_trade_sizes: VecDeque<f64>,
    #[serde(skip)] recent_book_imbalance: VecDeque<f64>,
    #[serde(skip)] recent_liquidation_flow: VecDeque<(i64,f64)>,
    #[serde(skip)] recent_event_ts_ms: VecDeque<i64>,
    #[serde(skip)] feature_history: BTreeMap<MarketFeature, VecDeque<f64>>,
    #[serde(skip)] feature_history_last_ts_ms: BTreeMap<MarketFeature, i64>,
}
#[derive(Clone)]
struct AppState {
    stream: broadcast::Sender<serde_json::Value>,
    database: mongodb::Database,
    events: Collection<Document>,
    states: Collection<Document>,
    market_vectors: Collection<Document>,
    variable_hubs: Collection<Document>,
    variable_hub_locks: Arc<Vec<tokio::sync::Mutex<()>>>,
    decisions: Collection<Document>,
    strategies: Collection<Document>,
    relationship_ledger: Collection<Document>,
    orders: Collection<Document>,
    trading_controls: Collection<Document>,
    memory: Arc<RwLock<HashMap<String, SymbolState>>>,
    symbol_ingest_locks: Arc<Vec<tokio::sync::Mutex<()>>>,
    active_strategy: Arc<RwLock<EdgeStrategy>>,
    signal_config: EngineConfig,
    signal_config_version: String,
    recovery_replayed_events: u64,
    recovery_skipped_events: u64,
}
#[derive(Deserialize)]
struct SignalRequest { observation: ForceObservation, has_open_position: bool }

#[derive(Deserialize)]
struct SymbolQuery { symbol: Option<String>, venue: Option<String> }
#[derive(Deserialize)]
struct StreamQuery { symbol: Option<String>, symbols: Option<String> }

fn timestamps_are_not_future(event_time_ms:i64,available_at_ms:i64)->bool{
    let now=chrono::Utc::now().timestamp_millis();
    event_time_ms<=now && available_at_ms<=now
}
fn valid_number(value: Option<f64>) -> bool { value.map(f64::is_finite).unwrap_or(true) }
fn validate_event(e: &MarketEvent) -> Result<(), String> {
    if e.event_id.trim().is_empty() || e.venue.trim().is_empty() || e.symbol.trim().is_empty() {
        return Err("event_id, venue, and symbol are required".into());
    }
    if e.event_id.len() > 256 || e.venue.len() > 64 || e.symbol.len() > 96 {
        return Err("event_id, venue, or symbol exceeds its maximum length".into());
    }
    if !matches!(e.event_type.as_str(), "trade" | "book" | "funding" | "open_interest" | "liquidation") {
        return Err("event_type must be trade, book, funding, open_interest, or liquidation".into());
    }
    if e.side.as_deref().is_some_and(|side| !matches!(side, "buy" | "sell")) {
        return Err("side must be buy or sell when supplied".into());
    }
    if e.liquidation_side.as_deref().is_some_and(|side| !matches!(side, "long" | "short")) {
        return Err("liquidation_side must be long or short when supplied".into());
    }
    if e.ts_ms <= 0 { return Err("ts_ms must be a positive Unix timestamp in milliseconds".into()); }
    if e.ts_ms > chrono::Utc::now().timestamp_millis() {
        return Err("future-dated market events are rejected to prevent look-ahead".into());
    }
    for (name, value) in [("price",e.price),("size",e.size),("bid",e.bid),("ask",e.ask),
        ("bid_size",e.bid_size),("ask_size",e.ask_size),("funding_rate",e.funding_rate),("open_interest",e.open_interest),
        ("mark_price",e.mark_price),("index_price",e.index_price)] {
        if !valid_number(value) { return Err(format!("{name} must be finite")); }
    }
    if let (Some(bid),Some(ask))=(e.bid,e.ask) {
        if bid < 0.0 || ask < bid { return Err("book prices must be non-negative and ask >= bid".into()); }
    }
    if [e.price,e.size,e.bid,e.ask,e.bid_size,e.ask_size,e.open_interest,e.mark_price,e.index_price].into_iter().flatten().any(|v| v < 0.0) {
        return Err("prices, sizes, and open interest must be non-negative".into());
    }
    Ok(())
}
fn push_bounded<T>(history: &mut VecDeque<T>, value: T) {
    push_bounded_limit(history,value,256);
}
fn push_bounded_limit<T>(history:&mut VecDeque<T>,value:T,limit:usize){
    history.push_back(value);
    while history.len()>limit{history.pop_front();}
}

fn apply(state: &mut SymbolState, event: &MarketEvent) {
    state.venue = event.venue.clone();
    state.symbol = event.symbol.clone();
    push_bounded_limit(&mut state.recent_event_ts_ms,event.ts_ms,4096);
    if let Some(price) = event.price.filter(|v| *v > 0.0) {
        if let Some(previous) = state.last_price.filter(|p| *p > 0.0) {
            let log_return = (price / previous).ln();
            state.ewma_abs_return = 0.94 * state.ewma_abs_return + 0.06 * log_return.abs();
            push_bounded(&mut state.recent_returns, log_return);
            push_bounded_limit(&mut state.recent_return_observations,(event.ts_ms,log_return),4096);
        }
        state.last_price = Some(price);
        state.last_price_ts_ms = Some(event.ts_ms);
        push_bounded(&mut state.recent_prices, price);
    }
    if let (Some(bid),Some(ask))=(event.bid,event.ask) {
        let mid=(bid+ask)/2.0;
        state.spread_bps=if mid>0.0 {Some((ask-bid)/mid*10_000.0)} else {None};
        state.last_book_ts_ms = Some(event.ts_ms);
        if let (Some(bs),Some(as_))=(event.bid_size,event.ask_size) {
            state.order_book_depth = Some(bs + as_);
            if bs+as_>0.0 {
                push_bounded(&mut state.recent_book_imbalance,(bs-as_)/(bs+as_));
            }
        }
    }
    match event.event_type.as_str() {
        "trade" => {
            state.last_trade_ts_ms = Some(event.ts_ms);
            let size=event.size.unwrap_or(0.0);
            let signed = match event.side.as_deref() { Some("buy")=>size, Some("sell")=>-size, _=>0.0 };
            state.signed_flow += signed;
            state.total_trade_size += size;
            push_bounded(&mut state.recent_trade_flow, signed);
            push_bounded(&mut state.recent_trade_sizes, size);
            state.event_intensity += 1.0;
        }
        "liquidation" => {
            state.last_liquidation_ts_ms = Some(event.ts_ms);
            state.liquidation_events+=1;
            state.event_intensity+=2.0;
            state.last_liquidation_side=event.liquidation_side.clone();
            let size=event.size.unwrap_or(0.0);
            let signed=match event.liquidation_side.as_deref(){Some("long")=>-size,Some("short")=>size,_=>0.0};
            push_bounded_limit(&mut state.recent_liquidation_flow,(event.ts_ms,signed),4096);
        }
        "funding" => {
            if event.funding_rate.is_some(){state.funding_rate=event.funding_rate;state.last_funding_ts_ms=Some(event.ts_ms);}
            if let (Some(mark),Some(index))=(event.mark_price,event.index_price){
                if mark>0.0&&index>0.0{
                    state.mark_price=Some(mark);
                    state.index_price=Some(index);
                    state.last_basis_ts_ms=Some(event.ts_ms);
                }
            }
        },
        "open_interest" => if let Some(value)=event.open_interest {
            state.last_open_interest_ts_ms=Some(event.ts_ms);
            state.open_interest_change=state.open_interest.filter(|previous|*previous>0.0)
                .map(|previous|(value-previous)/previous);
            state.open_interest=Some(value);
        },
        "book" => {},
        _ => {}
    }
    state.event_count+=1;
    state.last_event_ts_ms=Some(state.last_event_ts_ms.unwrap_or(0).max(event.ts_ms));
    // Keep the legacy decision inputs on the same bounded rolling horizon as
    // the vector features; lifetime totals otherwise make event intensity and
    // flow ratios drift permanently with process uptime.
    state.signed_flow=state.recent_trade_flow.iter().sum::<f64>();
    state.total_trade_size=state.recent_trade_sizes.iter().sum::<f64>();
    let trade_imbalance=if state.total_trade_size>0.0{
        Some((state.signed_flow/state.total_trade_size).clamp(-1.0,1.0))
    }else{None};
    let book_imbalance=mean(&state.recent_book_imbalance.iter().copied().collect::<Vec<_>>());
    state.order_flow_imbalance=match (trade_imbalance,book_imbalance){
        (Some(trade),Some(book))=>(trade+book)/2.0,
        (Some(trade),None)=>trade,
        (None,Some(book))=>book,
        (None,None)=>0.0,
    };
    state.event_intensity=state.recent_event_ts_ms.iter()
        .filter(|timestamp|event.ts_ms.saturating_sub(**timestamp)<=60_000).count() as f64;
    state.liquidation_events=state.recent_liquidation_flow.iter()
        .filter(|(timestamp,_)|event.ts_ms.saturating_sub(*timestamp)<=60_000).count();
    state.state_quality=((state.event_count as f64/100.0).sqrt()).min(1.0);
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() { None } else { Some(values.iter().sum::<f64>()/values.len() as f64) }
}
fn standard_deviation(values: &[f64]) -> Option<f64> {
    let center=mean(values)?;
    Some((values.iter().map(|value|(value-center).powi(2)).sum::<f64>()/values.len() as f64).sqrt())
}
fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty(){return None;}
    values.sort_by(f64::total_cmp);
    let middle=values.len()/2;
    Some(if values.len()%2==0 {(values[middle-1]+values[middle])/2.0}else{values[middle]})
}
/// Past-only robust normalization. The current raw value is appended only after
/// the historical center and scale have been calculated.
fn causal_robust_normalize(raw:f64,past:&VecDeque<f64>)->Option<f64>{
    if past.len()<8 || !raw.is_finite(){return None;}
    let mut ordered=past.iter().copied().collect::<Vec<_>>();
    let center=median(&mut ordered)?;
    let mut deviations=past.iter().map(|value|(value-center).abs()).collect::<Vec<_>>();
    let mut scale=1.4826*median(&mut deviations)?;
    if scale<=1.0e-12 {scale=standard_deviation(&ordered).unwrap_or(0.0);}
    if !scale.is_finite() || scale<=1.0e-12{return None;}
    Some(((raw-center)/scale).tanh().clamp(-1.0,1.0))
}

fn raw_feature_values(state:&SymbolState)->BTreeMap<MarketFeature,f64>{
    use MarketFeature::*;
    let mut output=BTreeMap::new();
    let prices=state.recent_prices.iter().copied().collect::<Vec<_>>();
    let returns=state.recent_returns.iter().copied().collect::<Vec<_>>();
    let price_window=&prices[prices.len().saturating_sub(64)..];
    let return_window=&returns[returns.len().saturating_sub(64)..];
    let volatility=standard_deviation(return_window);
    let average_return=mean(return_window);
    let persistence=if return_window.is_empty(){None}else{
        let up=return_window.iter().filter(|value|**value>0.0).count() as f64;
        let down=return_window.iter().filter(|value|**value<0.0).count() as f64;
        Some((up-down)/return_window.len() as f64)
    };
    if let Some(value)=return_window.last(){output.insert(LogReturn,*value);output.insert(GapMagnitude,value.abs());}
    if let (Some(current),Some(avg))=(price_window.last(),mean(price_window)){if avg>0.0{output.insert(PriceDisplacement,current/avg-1.0);}}
    if price_window.len()>=6{let old=price_window[price_window.len()-6];if old>0.0{output.insert(RateOfChange,(price_window[price_window.len()-1]/old).ln());}}
    if let (Some(current),Some(low),Some(high))=(price_window.last(),price_window.iter().copied().reduce(f64::min),price_window.iter().copied().reduce(f64::max)){
        if high>low{output.insert(SwingPosition,2.0*(current-low)/(high-low)-1.0);}
    }
    if let Some(vol)=volatility{
        output.insert(RealizedVolatility,vol);
        let directional_signal=average_return.unwrap_or(0.0).abs();
        output.insert(RegimeUncertainty,vol/(vol+directional_signal+1.0e-12));
        let abs_returns=return_window.iter().map(|value|value.abs()).collect::<Vec<_>>();
        if let Some(vov)=standard_deviation(&abs_returns){output.insert(VolatilityOfVolatility,vov);}
        if vol>1.0e-12 && return_window.len()>=4{
            let center=average_return.unwrap_or(0.0);
            output.insert(ReturnSkew,return_window.iter().map(|value|((value-center)/vol).powi(3)).sum::<f64>()/return_window.len() as f64);
            output.insert(ReturnKurtosis,return_window.iter().map(|value|((value-center)/vol).powi(4)).sum::<f64>()/return_window.len() as f64-3.0);
            output.insert(JumpIntensity,return_window.iter().filter(|value|value.abs()>3.0*vol).count() as f64/return_window.len() as f64);
        }
        if let Some(avg)=average_return{output.insert(TrendStrength,avg/vol.max(1.0e-12)*(return_window.len() as f64).sqrt());}
        if let Some(strength)=output.get(&TrendStrength).copied(){output.insert(RegimeTrendStrength,strength);}
    }
    if let Some(avg)=average_return{output.insert(Momentum,avg*return_window.len() as f64);}
    if let Some(value)=persistence{
        output.insert(TrendPersistence,value);output.insert(TrendExhaustion,1.0-value.abs());
        output.insert(RegimePersistence,value);
    }
    if return_window.len()>=4{
        let left=&return_window[..return_window.len()-1];let right=&return_window[1..];
        if let (Some(lm),Some(rm))=(mean(left),mean(right)){
            let covariance=left.iter().zip(right).map(|(a,b)|(a-lm)*(b-rm)).sum::<f64>()/left.len() as f64;
            let scale=standard_deviation(left).unwrap_or(0.0)*standard_deviation(right).unwrap_or(0.0);
            if scale>1.0e-12{output.insert(MeanReversionStrength,-covariance/scale);}
        }
    }
    if let Some(spread)=state.spread_bps{output.insert(SpreadBps,spread);output.insert(SlippageEstimate,spread/2.0);output.insert(RegimeLiquidityStress,spread);}
    if let Some(depth)=state.order_book_depth.filter(|value|*value>0.0){
        output.insert(OrderBookDepth,depth.ln_1p());output.insert(LiquidityConcentration,1.0/(1.0+depth.ln_1p()));
    }
    let recent_flow=state.recent_trade_flow.iter().sum::<f64>();let recent_size=state.recent_trade_sizes.iter().sum::<f64>();
    let trade_imbalance=if recent_size>0.0{Some((recent_flow/recent_size).clamp(-1.0,1.0))}else{None};
    let sizes=state.recent_trade_sizes.iter().copied().collect::<Vec<_>>();
    if sizes.len()>=16{
        let split=sizes.len()-8;
        let prior_volume=sizes[split-8..split].iter().sum::<f64>();
        let current_volume=sizes[split..].iter().sum::<f64>();
        let total=prior_volume+current_volume;
        if total>0.0{output.insert(VolumeDelta,(current_volume-prior_volume)/total);}
    }
    let book_imbalance=mean(&state.recent_book_imbalance.iter().copied().collect::<Vec<_>>());
    if let Some(value)=match (trade_imbalance,book_imbalance){
        (Some(trade),Some(book))=>Some((trade+book)/2.0),(Some(trade),None)=>Some(trade),(None,Some(book))=>Some(book),_=>None
    }{output.insert(OrderFlowImbalance,value.clamp(-1.0,1.0));}
    if let (Some(spread),Some(depth))=(state.spread_bps,state.order_book_depth){
        output.insert(MarketImpact,spread/(1.0+depth.max(0.0).sqrt()));
    }
    if let Some(value)=state.funding_rate{output.insert(FundingPressure,value);}
    if let Some(value)=state.open_interest_change{output.insert(OpenInterestChange,value);}
    if let (Some(mark),Some(index))=(state.mark_price,state.index_price){
        if index>0.0{output.insert(Basis,mark/index-1.0);}
    }
    let cutoff=state.last_event_ts_ms.unwrap_or_default();
    let recent_liquidations=state.recent_liquidation_flow.iter()
        .filter(|(timestamp,_)|cutoff.saturating_sub(*timestamp)<=60_000)
        .map(|(_,value)|*value).collect::<Vec<_>>();
    let liquidation_abs=recent_liquidations.iter().map(|value|value.abs()).sum::<f64>();
    if liquidation_abs>0.0{
        output.insert(LiquidationPressure,(recent_liquidations.iter().sum::<f64>()/liquidation_abs).clamp(-1.0,1.0));
    }else if state.venue=="binance_usdm"&&state.last_event_ts_ms.is_some(){
        // The Binance combined stream subscribes to forceOrder alongside trades,
        // book and mark-price updates. A live observation window with no force
        // orders is an observed zero, not a missing liquidation feature.
        output.insert(LiquidationPressure,0.0);
    }
    if let Some(cutoff)=state.last_event_ts_ms{
        let count=state.recent_event_ts_ms.iter().filter(|timestamp|cutoff.saturating_sub(**timestamp)<=60_000).count();
        output.insert(EventIntensity,count as f64);
    }
    if let Some(vol)=volatility{output.insert(RegimeVolatilityLevel,vol);}
    if return_window.len()>=12{
        let split=return_window.len()-6;
        if let (Some(prior),Some(current))=(standard_deviation(&return_window[split-6..split]),standard_deviation(&return_window[split..])){
            output.insert(RegimeTransitionIntensity,(current-prior).abs());
        }
    }
    if let Some(value)=persistence{output.insert(SignalDecay,1.0-value.abs());}
    output
}

fn minute_return_buckets(state:&SymbolState,cutoff_ms:i64)->BTreeMap<i64,f64>{
    let mut buckets=BTreeMap::new();
    let current_bucket=cutoff_ms.div_euclid(60_000);
    for (timestamp,log_return) in &state.recent_return_observations{
        let bucket=timestamp.div_euclid(60_000);
        // Only completed minute buckets are eligible. This avoids partial-bar
        // and same-timestamp ordering leakage across instruments.
        if bucket<current_bucket&&*timestamp<=cutoff_ms
            && cutoff_ms.saturating_sub(*timestamp)<=24*60*60*1_000{
            *buckets.entry(bucket).or_insert(0.0)+=*log_return;
        }
    }
    buckets
}

fn pearson_correlation(left:&[f64],right:&[f64])->Option<(f64,f64,f64)>{
    if left.len()!=right.len()||left.len()<2{return None;}
    let left_mean=mean(left)?;
    let right_mean=mean(right)?;
    let covariance=left.iter().zip(right).map(|(a,b)|(a-left_mean)*(b-right_mean)).sum::<f64>()/left.len() as f64;
    let left_variance=left.iter().map(|value|(value-left_mean).powi(2)).sum::<f64>()/left.len() as f64;
    let right_variance=right.iter().map(|value|(value-right_mean).powi(2)).sum::<f64>()/right.len() as f64;
    if left_variance<=1.0e-18||right_variance<=1.0e-18{return None;}
    let correlation=(covariance/(left_variance*right_variance).sqrt()).clamp(-1.0,1.0);
    let beta=covariance/right_variance;
    Some((correlation,covariance,beta))
}

/// Cross-asset features use same-venue, event-time-aligned one-minute return
/// buckets. No index-order alignment or cross-venue price mixing is permitted.
fn cross_asset_raw_features(state:&SymbolState,peers:&[SymbolState],cutoff_ms:i64)->BTreeMap<MarketFeature,f64>{
    use MarketFeature::*;
    if state.last_price_ts_ms.is_none_or(|timestamp|timestamp>cutoff_ms
        ||cutoff_ms.saturating_sub(timestamp)>5*60_000){
        return BTreeMap::new();
    }
    let current=minute_return_buckets(state,cutoff_ms);
    let mut correlations=Vec::new();
    let mut covariances=Vec::new();
    let mut betas=Vec::new();
    let mut breakdowns=Vec::new();
    let mut relative_strengths=Vec::new();
    for peer in peers{
        if peer.venue!=state.venue||peer.symbol==state.symbol
            || peer.last_price_ts_ms.is_none_or(|timestamp|cutoff_ms.saturating_sub(timestamp)>5*60_000){
            continue;
        }
        let other=minute_return_buckets(peer,cutoff_ms);
        let common=current.keys().filter_map(|bucket|{
            Some((*current.get(bucket)?,*other.get(bucket)?))
        }).collect::<Vec<_>>();
        if common.len()<12{continue;}
        let left=common.iter().map(|pair|pair.0).collect::<Vec<_>>();
        let right=common.iter().map(|pair|pair.1).collect::<Vec<_>>();
        let Some((correlation,covariance,beta))=pearson_correlation(&left,&right) else{continue;};
        let split=left.len()/2;
        let early=pearson_correlation(&left[..split],&right[..split]).map(|value|value.0);
        let late=pearson_correlation(&left[split..],&right[split..]).map(|value|value.0);
        let Some((early,late))=early.zip(late) else{continue;};
        correlations.push(correlation);
        covariances.push(covariance);
        betas.push(beta);
        breakdowns.push((late-early).abs().clamp(0.0,2.0)/2.0);
        relative_strengths.push(mean(&left).unwrap_or(0.0)-mean(&right).unwrap_or(0.0));
    }
    let mut output=BTreeMap::new();
    if !correlations.is_empty(){
        output.insert(Correlation,mean(&correlations).unwrap_or(0.0));
        output.insert(Covariance,mean(&covariances).unwrap_or(0.0));
        output.insert(Beta,mean(&betas).unwrap_or(0.0));
        output.insert(CorrelationBreakdown,mean(&breakdowns).unwrap_or(0.0));
        output.insert(RelativeStrength,mean(&relative_strengths).unwrap_or(0.0));
        output.insert(RegimeCorrelationStress,mean(&correlations.iter().map(|value|value.abs()).collect::<Vec<_>>()).unwrap_or(0.0));
    }
    output
}

fn cross_asset_source_timestamp(state:&SymbolState,peers:&[SymbolState],cutoff_ms:i64)->Option<i64>{
    let current_bucket=cutoff_ms.div_euclid(60_000);
    if state.last_price_ts_ms.is_none_or(|timestamp|timestamp>cutoff_ms
        ||cutoff_ms.saturating_sub(timestamp)>5*60_000){
        return None;
    }
    let has_peer=peers.iter().any(|peer|peer.venue==state.venue
        && peer.symbol!=state.symbol
        && peer.last_price_ts_ms.is_some_and(|timestamp|timestamp<=cutoff_ms
            && cutoff_ms.saturating_sub(timestamp)<=5*60_000));
    has_peer.then_some(current_bucket*60_000)
}

fn bounded_identity_feature(feature:MarketFeature)->bool{
    use MarketFeature::*;
    matches!(feature,
        SwingPosition|TrendPersistence|MeanReversionStrength|TrendExhaustion|
        OrderFlowImbalance|VolumeDelta|LiquidityConcentration|Correlation|
        CorrelationBreakdown|LiquidationPressure|JumpIntensity|RegimeUncertainty|
        RegimePersistence|RegimeCorrelationStress|SignalDecay
    )
}

fn fixed_scale_feature(feature:MarketFeature,raw:f64)->Option<f64>{
    use MarketFeature::*;
    let scale=match feature{
        FundingPressure=>10_000.0,
        OpenInterestChange=>20.0,
        Basis=>1_000.0,
        _=>return None,
    };
    raw.is_finite().then(||(raw*scale).tanh())
}

fn feature_source_timestamp(state:&SymbolState,feature:MarketFeature,cross_timestamp:Option<i64>)->Option<i64>{
    use MarketFeature::*;
    match feature{
        LogReturn|PriceDisplacement|RateOfChange|SwingPosition|GapMagnitude|RealizedVolatility|
        VolatilityOfVolatility|ReturnSkew|ReturnKurtosis|JumpIntensity|Momentum|TrendStrength|
        TrendPersistence|MeanReversionStrength|TrendExhaustion|RegimeTrendStrength|
        RegimeVolatilityLevel|RegimeTransitionIntensity|RegimeUncertainty|RegimePersistence|SignalDecay
            =>state.last_price_ts_ms,
        OrderFlowImbalance=>match(state.last_trade_ts_ms,state.last_book_ts_ms){
            (Some(trade),Some(book))=>Some(trade.max(book)),
            (Some(trade),None)=>Some(trade),
            (None,Some(book))=>Some(book),
            _=>None,
        },
        VolumeDelta=>state.last_trade_ts_ms,
        SpreadBps|OrderBookDepth|MarketImpact|SlippageEstimate|LiquidityConcentration|
        RegimeLiquidityStress=>state.last_book_ts_ms,
        FundingPressure=>state.last_funding_ts_ms,
        OpenInterestChange=>state.last_open_interest_ts_ms,
        Basis=>state.last_basis_ts_ms,
        LiquidationPressure=>if state.venue=="binance_usdm"{state.last_event_ts_ms}else{state.last_liquidation_ts_ms},
        EventIntensity=>state.last_event_ts_ms,
        Correlation|Covariance|Beta|CorrelationBreakdown|RelativeStrength|RegimeCorrelationStress=>cross_timestamp,
        _=>None,
    }
}

fn feature_freshness(state:&SymbolState,feature:MarketFeature,cutoff_ms:i64,cross_timestamp:Option<i64>)->Option<f64>{
    use MarketFeature::*;
    let (timestamp,max_age_ms)=match feature{
        LogReturn|PriceDisplacement|RateOfChange|SwingPosition|GapMagnitude|RealizedVolatility|
        VolatilityOfVolatility|ReturnSkew|ReturnKurtosis|JumpIntensity|Momentum|TrendStrength|
        TrendPersistence|MeanReversionStrength|TrendExhaustion|RegimeTrendStrength|
        RegimeVolatilityLevel|RegimeTransitionIntensity|RegimeUncertainty|RegimePersistence|SignalDecay
            =>(state.last_price_ts_ms,60_000_i64),
        VolumeDelta=>(state.last_trade_ts_ms,60_000_i64),
        OrderFlowImbalance=>{
            let timestamp=match(state.last_trade_ts_ms,state.last_book_ts_ms){
                (Some(trade),Some(book))=>Some(trade.max(book)),
                (Some(trade),None)=>Some(trade),
                (None,Some(book))=>Some(book),
                _=>None,
            };
            (timestamp,60_000_i64)
        },
        SpreadBps|OrderBookDepth|MarketImpact|SlippageEstimate|LiquidityConcentration|
        RegimeLiquidityStress=>(state.last_book_ts_ms,30_000_i64),
        FundingPressure=>(state.last_funding_ts_ms,8*60*60*1_000_i64),
        OpenInterestChange=>(state.last_open_interest_ts_ms,5*60*1_000_i64),
        Basis=>(state.last_basis_ts_ms,60_000_i64),
        LiquidationPressure=>(if state.venue=="binance_usdm"{state.last_event_ts_ms}else{state.last_liquidation_ts_ms},60_000_i64),
        EventIntensity=>(state.last_event_ts_ms,60_000_i64),
        Correlation|Covariance|Beta|CorrelationBreakdown|RelativeStrength|RegimeCorrelationStress=>(cross_timestamp,5*60_000_i64),
        _=>return None,
    };
    let timestamp=timestamp?;
    let age_ms=cutoff_ms.saturating_sub(timestamp).max(0);
    if age_ms>max_age_ms{return None;}
    Some((-(age_ms as f64)/(max_age_ms as f64)).exp())
}

fn build_market_vector(state:&mut SymbolState,event:&MarketEvent,available_at_ms:i64,peers:&[SymbolState])->Option<MarketVector>{
    let cutoff=state.last_event_ts_ms?;
    let identity=format!("{}|{}|{}|event-window|{}",event.venue,event.symbol,event.event_id,cutoff);
    let digest=hex::encode(Sha256::digest(identity.as_bytes()));
    let vector_id=format!("mv-{}",&digest[..32]);
    let mut vector=MarketVector::new(vector_id,&event.venue,&event.symbol,"event-window",cutoff,available_at_ms.max(cutoff)).ok()?;
    // These dimensions are outside the current per-symbol provider contract.
    // They are not neutral observations and must not dilute missing-data quality.
    for feature in [
        MarketFeature::ImpliedVolatility,
        MarketFeature::GammaExposure,
        MarketFeature::DeltaExposure,
        MarketFeature::PortfolioExposure,
        MarketFeature::Concentration,
        MarketFeature::Drawdown,
        MarketFeature::TailRisk,
        MarketFeature::RiskContribution,
    ]{
        vector.set_not_applicable(feature);
    }
    if event.venue!="binance_usdm"{
        // Current Coinbase/Kraken spot and Finnhub feeds have no perpetual
        // funding, open-interest, liquidation or paired index/derivative basis.
        for feature in [
            MarketFeature::FundingPressure,
            MarketFeature::OpenInterestChange,
            MarketFeature::LiquidationPressure,
            MarketFeature::Basis,
        ]{
            vector.set_not_applicable(feature);
        }
    }
    let mut raw_features=raw_feature_values(state);
    raw_features.extend(cross_asset_raw_features(state,peers,cutoff));
    let cross_timestamp=cross_asset_source_timestamp(state,peers,cutoff);
    let mut observed=0usize;
    for (feature,raw) in raw_features{
        let Some(freshness)=feature_freshness(state,feature,cutoff,cross_timestamp) else {continue;};
        let source_timestamp=feature_source_timestamp(state,feature,cross_timestamp)?;
        let history=state.feature_history.entry(feature).or_default();
        let history_quality=((history.len()+1) as f64/32.0).min(1.0);
        let normalized=if bounded_identity_feature(feature){
            if raw.is_finite()&&(-1.0..=1.0).contains(&raw){
                Some((raw,history_quality))
            }else{None}
        }else if let Some(value)=fixed_scale_feature(feature,raw){
            Some((value,history_quality))
        }else{
            causal_robust_normalize(raw,history)
                .map(|value|(value,(history.len() as f64/32.0).min(1.0)))
        };
        if let Some((value,calibration_quality))=normalized{
            let quality=state.state_quality.min(calibration_quality)*freshness;
            if quality>0.0 && vector.set_value(feature,Some(value),quality).is_ok(){observed+=1;}
        }
        // A book/funding event can trigger a new vector without creating a new
        // independent observation for a stale price feature. Avoid overweighting
        // repeated values in the rolling calibration history.
        if state.feature_history_last_ts_ms.get(&feature).copied()!=Some(source_timestamp){
            push_bounded(history,raw);
            state.feature_history_last_ts_ms.insert(feature,source_timestamp);
        }
    }
    if observed==0{None}else{Some(vector)}
}

/// Evaluate the actual V1 vector engine on the immutable snapshot produced by
/// ingestion. Missing required coordinates are reported explicitly; the
/// adapter never substitutes a neutral zero for unavailable market evidence.
fn evaluate_ingested_v1_vector(
    vector:&MarketVector,
    config:&EngineConfig,
    config_version:&str,
)->serde_json::Value{
    const REQUIRED:[MarketFeature;12]=[
        MarketFeature::OrderFlowImbalance,
        MarketFeature::OrderBookDepth,
        MarketFeature::RealizedVolatility,
        MarketFeature::FundingPressure,
        MarketFeature::OpenInterestChange,
        MarketFeature::LiquidationPressure,
        MarketFeature::Basis,
        MarketFeature::RegimeVolatilityLevel,
        MarketFeature::RegimeLiquidityStress,
        MarketFeature::RegimeCorrelationStress,
        MarketFeature::RegimeTransitionIntensity,
        MarketFeature::RegimeUncertainty,
    ];
    let unavailable=REQUIRED.iter().filter_map(|feature|{
        let coordinate=vector.coordinates.get(feature)?;
        (coordinate.status!=FeatureAvailability::Observed
            && !( *feature==MarketFeature::Basis && coordinate.status==FeatureAvailability::NotApplicable)
            || coordinate.status==FeatureAvailability::Observed && coordinate.value.is_none())
            .then(||serde_json::json!({
                "feature":feature.as_str(),
                "status":coordinate.status,
                "quality":coordinate.quality
            }))
    }).collect::<Vec<_>>();
    if !unavailable.is_empty(){
        return serde_json::json!({
            "status":"not_ready",
            "algorithm_version":"1",
            "vector_id":vector.vector_id,
            "schema_version":vector.schema_version,
            "missing_required_features":unavailable,
            "reason":"V1 abstains until every required force and regime coordinate is observed"
        });
    }
    match decide_from_market_vector(vector,config,config_version,false){
        Ok(decision)=>serde_json::json!({
            "status":"evaluated",
            "algorithm_version":"1",
            "vector_id":vector.vector_id,
            "schema_version":vector.schema_version,
            "decision":decision
        }),
        Err(error)=>serde_json::json!({
            "status":"error",
            "algorithm_version":"1",
            "vector_id":vector.vector_id,
            "schema_version":vector.schema_version,
            "reason":error.to_string()
        })
    }
}
fn should_evaluate_v1(last_evaluated_at_ms:Option<i64>,current_event_ts_ms:i64)->bool{
    last_evaluated_at_ms.is_none_or(|previous|{
        previous.div_euclid(1_000)<current_event_ts_ms.div_euclid(1_000)
    })
}

fn cached_v1_decision(state:&SymbolState,current_event_ts_ms:i64)->serde_json::Value{
    match state.last_v1_decision.clone(){
        Some(result) if state.last_v1_evaluated_at_ms==Some(current_event_ts_ms)=>result,
        Some(result)=>serde_json::json!({
            "status":"cached",
            "algorithm_version":"1",
            "evaluated_at_ms":state.last_v1_evaluated_at_ms,
            "result":result
        }),
        None=>serde_json::json!({
            "status":"warming_up",
            "algorithm_version":"1",
            "reason":"past-only feature calibration has not produced a V1 evaluation"
        })
    }
}

fn market_event_decision_key(event:&MarketEvent)->String{
    let identity=format!("{}|{}|{}",event.venue,event.symbol,event.event_id);
    hex::encode(Sha256::digest(identity.as_bytes()))
}

fn pending_pipeline_event(document:&Document)->bool{
    document.get_str("_qoin_pipeline_status").ok()==Some("pending")
}

fn should_run_v2_history_discovery(previous_ts_ms:Option<i64>,current_ts_ms:i64)->bool{
    previous_ts_ms.is_some_and(|previous|{
        previous.div_euclid(5*60_000)<current_ts_ms.div_euclid(5*60_000)
    })
}

fn persisted_event_is_applied(document:&Document)->bool{
    // Legacy event rows predate the explicit marker and are treated as applied.
    document.get_bool("_qoin_applied").unwrap_or(true)
}

fn constant_time_token_match(expected: &[u8], supplied: &[u8]) -> bool {
    if expected.len() != supplied.len() { return false; }
    expected.iter().zip(supplied).fold(0u8, |difference, (a, b)| difference | (a ^ b)) == 0
}

fn execution_token_authorized(headers: &HeaderMap) -> bool {
    let expected = match env::var("QOIN_EXECUTION_TOKEN") {
        Ok(token) if !token.trim().is_empty() => token,
        _ => return false,
    };
    let supplied = headers.get("x-qoin-execution-token").and_then(|value| value.to_str().ok()).unwrap_or("");
    constant_time_token_match(expected.as_bytes(), supplied.as_bytes())
}

#[derive(Deserialize)]
struct TradingControlRequest { enabled: bool }

async fn set_trading_enabled(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<TradingControlRequest>,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized"})));
    }
    let update = doc!{"$set":{"enabled":request.enabled,"updated_at":mongodb::bson::DateTime::now()}};
    match s.trading_controls.update_one(doc!{"_id":"execution"}, update,
        UpdateOptions::builder().upsert(true).build()).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({
            "execution_enabled":request.enabled,
            "message":if request.enabled{"operator enabled order submission"}else{"kill switch engaged; new orders blocked"}
        }))),
        Err(error) => {
            tracing::error!(error=%error, "could not update trading kill switch");
            (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"trading control store unavailable"})))
        }
    }
}

async fn submit_live_order(
    State(s): State<AppState>,
    headers: HeaderMap,
    Json(order): Json<trading::OrderRequest>,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized or execution token not configured"})));
    }
    let trading_enabled = match s.trading_controls.find_one(doc!{"_id":"execution"}, None).await {
        Ok(Some(control)) => control.get_bool("enabled").unwrap_or(false),
        Ok(None) => false,
        Err(error) => {
            tracing::error!(error=%error, "trading kill-switch lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"trading control unavailable; order rejected"})));
        }
    };
    if !trading_enabled {
        return (StatusCode::LOCKED, Json(serde_json::json!({"error":"trading kill switch is engaged; enable execution explicitly"})));
    }
    let executor = match trading::BinanceExecutor::from_env() {
        Ok(executor) => executor,
        Err(error) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":error}))),
    };
    let request_doc = match mongodb::bson::to_document(&order) {
        Ok(document) => document,
        Err(_) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"order serialization failed"}))),
    };
    let ledger = doc! {
        "client_order_id": &order.client_order_id,
        "status": "submitting",
        "request": request_doc,
        "created_at": mongodb::bson::DateTime::now(),
        "updated_at": mongodb::bson::DateTime::now(),
    };
    match s.orders.insert_one(ledger, None).await {
        Ok(_) => {},
        Err(error) if error.to_string().contains("E11000") => {
            let existing = s.orders.find_one(doc!{"client_order_id":&order.client_order_id}, None).await
                .ok().flatten().unwrap_or_default();
            let status = existing.get_str("status").unwrap_or("unknown");
            return (StatusCode::CONFLICT, Json(serde_json::json!({
                "error":"client_order_id has already been used; do not resubmit",
                "client_order_id":order.client_order_id,
                "existing_status":status,
                "reconciliation_required":status == "submitting" || status == "reconciliation_required"
            })));
        },
        Err(error) => {
            tracing::error!(error=%error, "could not persist durable order intent");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"durable order ledger unavailable; order not submitted"})));
        }
    }
    match executor.place_market_order(order.clone()).await {
        Ok(result) => {
            let response = mongodb::bson::to_bson(&result).unwrap_or(mongodb::bson::Bson::Null);
            let update = doc!{"$set":{
                "status":"accepted",
                "exchange_response":response,
                "updated_at":mongodb::bson::DateTime::now()
            }};
            if let Err(error) = s.orders.update_one(doc!{"client_order_id":&order.client_order_id}, update, None).await {
                tracing::error!(error=%error, client_order_id=%order.client_order_id,
                    "exchange accepted order but durable ledger update failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"exchange accepted order but ledger update failed; reconcile at exchange before retrying",
                    "client_order_id":order.client_order_id,
                    "reconciliation_required":true
                })));
            }
            (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default()))
        }
        Err(error) => {
            let update = doc!{"$set":{
                "status":"reconciliation_required",
                "execution_error":&error,
                "updated_at":mongodb::bson::DateTime::now()
            }};
            if let Err(ledger_error) = s.orders.update_one(doc!{"client_order_id":&order.client_order_id}, update, None).await {
                tracing::error!(error=%ledger_error, client_order_id=%order.client_order_id,
                    "execution failed and durable ledger update also failed");
            }
            tracing::error!(client_order_id=%order.client_order_id, error=%error,
                "live order execution requires reconciliation");
            (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
                "error":error,
                "client_order_id":order.client_order_id,
                "reconciliation_required":true
            })))
        }
    }
}

async fn reconcile_live_order(
    State(s): State<AppState>,
    Path(client_order_id): Path<String>,
    headers: HeaderMap,
) -> impl IntoResponse {
    if !execution_token_authorized(&headers) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error":"unauthorized or execution token not configured"})));
    }
    let ledger = match s.orders.find_one(doc!{"client_order_id":&client_order_id}, None).await {
        Ok(Some(record)) => record,
        Ok(None) => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"order not found"}))),
        Err(error) => {
            tracing::error!(error=%error, "order ledger lookup failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"order ledger unavailable"})));
        }
    };
    let request_doc = match ledger.get_document("request") {
        Ok(document) => document.clone(),
        Err(_) => return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored order request is malformed"}))),
    };
    let order = match mongodb::bson::from_document::<trading::OrderRequest>(request_doc) {
        Ok(order) => order,
        Err(_) => return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored order request cannot be decoded"}))),
    };
    let executor = match trading::BinanceExecutor::from_env() {
        Ok(executor) => executor,
        Err(error) => return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":error}))),
    };
    let entry = match executor.get_order_by_client_id(&order.symbol, &order.client_order_id).await {
        Ok(value) => value,
        Err(error) => return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({
            "error":"exchange entry-order lookup failed",
            "detail":error,
            "client_order_id":client_order_id,
            "reconciliation_required":true
        }))),
    };
    let stop = if order.reduce_only {
        serde_json::Value::Null
    } else {
        let stop_id = trading::protective_stop_client_order_id(&order.client_order_id);
        match executor.get_order_by_client_id(&order.symbol, &stop_id).await {
            Ok(value) => value,
            Err(error) => serde_json::json!({"lookup_error":error}),
        }
    };
    let entry_status = entry.get("status").and_then(|value| value.as_str()).unwrap_or("UNKNOWN");
    let stop_status = stop.get("status").and_then(|value| value.as_str()).unwrap_or("UNKNOWN");
    let reconciled = if order.reduce_only {
        matches!(entry_status, "FILLED" | "CANCELED" | "REJECTED" | "EXPIRED")
    } else {
        entry_status == "FILLED" && stop_status == "NEW"
    };
    let status = if reconciled { "reconciled" } else { "reconciliation_required" };
    if let Err(error) = s.orders.update_one(doc!{"client_order_id":&client_order_id},
        doc!{"$set":{"status":status,"exchange_entry":mongodb::bson::to_bson(&entry).unwrap_or(mongodb::bson::Bson::Null),
            "exchange_protective_stop":mongodb::bson::to_bson(&stop).unwrap_or(mongodb::bson::Bson::Null),
            "reconciled_at":mongodb::bson::DateTime::now(),"updated_at":mongodb::bson::DateTime::now()}}, None).await {
        tracing::error!(error=%error, client_order_id=%client_order_id, "could not persist exchange reconciliation result");
        return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"exchange queried but reconciliation result could not be persisted","client_order_id":client_order_id})));
    }
    (if reconciled {StatusCode::OK}else{StatusCode::CONFLICT}, Json(serde_json::json!({
        "client_order_id":client_order_id,
        "status":status,
        "entry":entry,
        "protective_stop":stop
    })))
}

async fn evaluate_signal(State(s): State<AppState>, Json(request): Json<SignalRequest>) -> Result<Json<Signal>, (StatusCode, String)> {
    let result = decide_signal(
        request.observation,
        &s.signal_config,
        &s.signal_config_version,
        request.has_open_position,
    ).map_err(|error| (StatusCode::BAD_REQUEST, error.to_string()))?;
    Ok(Json(result))
}

async fn health(State(s): State<AppState>) -> impl IntoResponse {
    let database_ok=s.database.run_command(doc!{"ping":1},None).await.is_ok();
    let pending_events=s.events.count_documents(doc!{"_qoin_pipeline_status":"pending"},None).await.unwrap_or(u64::MAX);
    let recovery_ok=s.recovery_skipped_events==0&&pending_events==0;
    let order_entry_enabled=s.trading_controls.find_one(doc!{"_id":"execution"},None).await
        .ok().flatten().and_then(|control|control.get_bool("enabled").ok()).unwrap_or(false);
    let healthy=database_ok && recovery_ok;
    let strategy_version=s.active_strategy.read().ok().map(|v|v.version.clone()).unwrap_or_else(||"unavailable".into());
    (if healthy {StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},
     Json(serde_json::json!({"service":"qoin-axum-flow-engine","status":if healthy{"ok"}else{"degraded"},
       "mongodb":database_ok,
       "market_recovery":{"status":if recovery_ok{"ok"}else{"degraded"},
         "replayed_events":s.recovery_replayed_events,"skipped_events":s.recovery_skipped_events,
         "pending_pipeline_events":pending_events},
       "strategy_version":strategy_version,
       "market_streams":{"binance_usdm_enabled":env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true),
          "coinbase_advanced_trade_enabled":env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false),
          "kraken_spot_enabled":env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false),
          "finnhub_key_configured":env::var("FINNHUB_API_KEY").map(|v|!v.trim().is_empty()).unwrap_or(false)},
       "live_order_submission":if matches!(env::var("QOIN_TRADING_MODE").as_deref(),Ok("live")|Ok("testnet"))
          && env::var("QOIN_EXECUTION_TOKEN").map(|v|!v.trim().is_empty()).unwrap_or(false)
          && env::var("BINANCE_API_KEY").map(|v|!v.trim().is_empty()).unwrap_or(false)
          && env::var("BINANCE_API_SECRET").map(|v|!v.trim().is_empty()).unwrap_or(false)
          && ((env::var("QOIN_TRADING_MODE").as_deref()==Ok("live")
                && env::var("QOIN_LIVE_TRADING_CONFIRM").map(|v|v=="I_UNDERSTAND_REAL_MONEY_RISK").unwrap_or(false))
              || (env::var("QOIN_TRADING_MODE").as_deref()==Ok("testnet")
                && env::var("QOIN_TESTNET_TRADING_CONFIRM").map(|v|v=="I_UNDERSTAND_TESTNET_ORDERS").unwrap_or(false)))
          {"configured"}else{"disabled"},
       "live_order_adapter":"binance_usdm_with_required_protective_stop",
       "new_order_entry_enabled":order_entry_enabled,
       "research_signal_api":"enabled", "research_signal_config_version":s.signal_config_version})))
}
async fn ingest(State(s):State<AppState>,Json(event):Json<MarketEvent>)->impl IntoResponse{
    // Serialize the full persistence/decision pipeline per canonical symbol.
    // The memory lock alone only protects state mutation; without this guard,
    // concurrent events could persist snapshots and decisions out of order.
    // A fixed set of lock stripes bounds memory even if malformed/internal
    // callers submit an unbounded number of distinct symbol strings.
    let state_key=format!("{}|{}",event.venue,event.symbol);
    let lock_stripe = state_key.bytes().fold(0usize, |hash, byte|
        hash.wrapping_mul(31).wrapping_add(byte as usize)) % s.symbol_ingest_locks.len();
    let _symbol_guard = s.symbol_ingest_locks[lock_stripe].lock().await;
    if let Err(message)=validate_event(&event){tracing::warn!(symbol=%event.symbol,event_type=%event.event_type,reason=%message,"invalid market event rejected");return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let latest_before_ingest=match s.memory.read(){
        Ok(memory)=>memory.get(&state_key).and_then(|state|state.last_event_ts_ms),
        Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})))
    };
    let late_event=latest_before_ingest.is_some_and(|last|event.ts_ms<last);
    let persist_snapshot=latest_before_ingest.is_none_or(|last|{
        last.div_euclid(1_000)<event.ts_ms.div_euclid(1_000)
    });
    let run_v2_discovery=should_run_v2_history_discovery(latest_before_ingest,event.ts_ms);
    let event_available_at_ms=chrono::Utc::now().timestamp_millis();
    let mut raw=match mongodb::bson::to_document(&event){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"event serialization failed"})))};
    // Preserve the original availability boundary so replayed vectors retain
    // their provenance and V2 ledger event IDs remain stable across restarts.
    raw.insert("_qoin_available_at_ms",event_available_at_ms);
    // The immutable log records whether the live state machine applied this
    // event. Recovery must preserve the same policy for late arrivals.
    raw.insert("_qoin_applied",!late_event);
    raw.insert("_qoin_pipeline_status",if late_event{"ignored_late"}else{"pending"});
    match s.events.insert_one(raw,None).await{
        Ok(_)=>{},Err(e) if e.to_string().contains("E11000")=>{
            let existing=match s.events.find_one(
                doc!{"venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id},None
            ).await{
                Ok(value)=>value,
                Err(error)=>{
                    tracing::error!(error=%error,event_id=%event.event_id,
                        "could not inspect duplicate event processing status");
                    return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
                        "error":"duplicate event status unavailable","event_id":event.event_id
                    })));
                }
            };
            let pending=existing.as_ref().is_some_and(pending_pipeline_event);
            return(
                if pending{StatusCode::ACCEPTED}else{StatusCode::OK},
                Json(serde_json::json!({
                    "status":if pending{"pending_recovery"}else{"duplicate_ignored"},
                    "event_id":event.event_id,
                    "recovery_required":pending
                }))
            );
        },
        Err(e)=>{tracing::error!(error=%e,"event persistence failed");return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"event persistence failed"})));}
    }
    if late_event{
        return(StatusCode::ACCEPTED,Json(serde_json::json!({"status":"late_event_persisted_not_applied","event_id":event.event_id,
            "event_ts_ms":event.ts_ms,"latest_state_ts_ms":latest_before_ingest})));
    }
    let (mut snapshot,market_vector,should_evaluate_v1_now)={
        let mut guard=match s.memory.write(){Ok(v)=>v,Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"state lock poisoned"})))};
        let venue_prefix=format!("{}|",event.venue);
        let peer_states=guard.iter()
            .filter(|(key,_)|key.starts_with(&venue_prefix)&&*key!=&state_key)
            .map(|(_,peer)|peer.clone()).collect::<Vec<_>>();
        let state=guard.entry(state_key.clone()).or_default();
        // The per-symbol ingest lock makes this a defensive invariant check.
        if state.last_event_ts_ms.is_some_and(|last|event.ts_ms<last){
            return(StatusCode::CONFLICT,Json(serde_json::json!({"error":"event became late before state application","event_persisted":true})));
        }
        apply(state,&event);
        let vector=build_market_vector(state,&event,event_available_at_ms,&peer_states);
        let should_evaluate=vector.is_some()&&should_evaluate_v1(state.last_v1_evaluated_at_ms,event.ts_ms);
        (state.clone(),vector,should_evaluate)
    };
    if should_evaluate_v1_now{
        let vector=market_vector.as_ref().expect("V1 evaluation requires a market vector");
        let mut document=match mongodb::bson::to_document(vector){Ok(value)=>value,Err(error)=>{
            tracing::error!(error=%error,"market vector serialization failed");
            return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"market vector serialization failed","event_persisted":true})));
        }};
        document.insert("stored_at",mongodb::bson::DateTime::now());
        if let Err(error)=s.market_vectors.insert_one(document,None).await{
            tracing::error!(error=%error,vector_id=%vector.vector_id,"market vector persistence failed");
            return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector persistence failed","event_persisted":true})));
        }
    }
    let strategy=match s.active_strategy.read(){Ok(v)=>v.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))};
    let mut decision:EdgeDecision=match strategy.decide(&snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
        snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,snapshot.liquidation_events,snapshot.last_liquidation_side.as_deref()){
        Ok(v)=>v,Err(e)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":e})))
    };
    decision.input_event = Some(DecisionInputEvent {
        event_id: event.event_id.clone(),
        venue: event.venue.clone(),
        event_type: event.event_type.clone(),
        event_ts_ms: event.ts_ms,
        state_event_count: snapshot.event_count,
        state_last_event_ts_ms: snapshot.last_event_ts_ms,
    });
    let v1_vector_decision=if should_evaluate_v1_now{
        let result=evaluate_ingested_v1_vector(
            market_vector.as_ref().expect("V1 evaluation requires a market vector"),
            &s.signal_config,&s.signal_config_version,
        );
        snapshot.last_v1_decision=Some(result.clone());
        snapshot.last_v1_evaluated_at_ms=Some(event.ts_ms);
        match s.memory.write(){
            Ok(mut memory)=>{
                if let Some(state)=memory.get_mut(&state_key){
                    state.last_v1_decision=Some(result.clone());
                    state.last_v1_evaluated_at_ms=Some(event.ts_ms);
                }
            }
            Err(_)=>tracing::error!(symbol=%event.symbol,"could not update cached V1 evaluation state"),
        }
        result
    }else{
        cached_v1_decision(&snapshot,event.ts_ms)
    };
    let persist_pipeline_snapshot=persist_snapshot||should_evaluate_v1_now;
    if persist_pipeline_snapshot{
        let mut state_doc=mongodb::bson::to_document(&snapshot).unwrap_or_default();
        state_doc.insert("stored_at",mongodb::bson::DateTime::now());
        if let Err(e)=s.states.insert_one(state_doc,None).await{
            tracing::error!(error=%e,"state snapshot persistence failed");
            return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state persistence failed","event_persisted":true})));
        }
    }
    let mut decision_doc=mongodb::bson::to_document(&decision).unwrap_or_default();
    decision_doc.insert("v1_shared_market_vector",mongodb::bson::to_bson(&v1_vector_decision).unwrap_or(mongodb::bson::Bson::Null));
    decision_doc.insert("decision_key",market_event_decision_key(&event));
    if should_evaluate_v1_now{
        if let Some(vector)=market_vector.as_ref(){decision_doc.insert("market_vector_id",&vector.vector_id);}
    }
    decision_doc.insert("stored_at",mongodb::bson::DateTime::now());
    if persist_pipeline_snapshot{
        if let Err(e)=s.decisions.insert_one(decision_doc,None).await{
            tracing::error!(error=%e,"decision persistence failed");
            return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision persistence failed","event_persisted":true,"state_persisted":persist_pipeline_snapshot})));
        }
    }
    let v2_discovery=if run_v2_discovery&&market_vector.is_some(){
        let query=DiscoverVectorHistoryQuery{
            venue:event.venue.clone(),symbol:event.symbol.clone(),limit:Some(6),temperature:Some(1.0),
        };
        match discover_vector_relationships_from_history(State(s.clone()),Query(query)).await{
            Ok((http_status,Json(mut result)))=>{
                result["http_status"]=serde_json::json!(http_status.as_u16());
                if let Some(vector)=market_vector.as_ref(){
                    if let Err(error)=s.decisions.update_one(
                        doc!{"market_vector_id":&vector.vector_id},
                        doc!{"$set":{"v2_relationship_discovery":mongodb::bson::to_bson(&result).unwrap_or(mongodb::bson::Bson::Null)}},
                        None,
                    ).await{
                        tracing::warn!(error=%error,vector_id=%vector.vector_id,
                            "V2 ledger append succeeded or warmed up but decision summary link failed");
                    }
                }
                serde_json::Value::Object(result.as_object().cloned().unwrap_or_default())
            }
            Err((http_status,Json(error)))=>{
                tracing::warn!(status=%http_status,error=?error,symbol=%event.symbol,
                    "automatic V2 history discovery did not complete");
                serde_json::json!({"status":"error","http_status":http_status.as_u16(),"error":error})
            }
        }
    }else{serde_json::Value::Null};
    if let Err(error)=s.events.update_one(
        doc!{"venue":&event.venue,"symbol":&event.symbol,"event_id":&event.event_id},
        doc!{"$set":{"_qoin_pipeline_status":"complete","_qoin_completed_at":mongodb::bson::DateTime::now()}},
        None,
    ).await{
        tracing::warn!(error=%error,event_id=%event.event_id,
            "market event processed but completion marker could not be persisted");
    }
    let update=serde_json::json!({"type":"market.update","symbol":snapshot.symbol,"event":event,
        "state":snapshot,"decision":decision,"v1_shared_market_vector":v1_vector_decision,
        "v2_relationship_discovery":v2_discovery,
        "market_vector":market_vector,"market_vector_persisted":should_evaluate_v1_now,
        "server_ts_ms":chrono::Utc::now().timestamp_millis()});
    let _=s.stream.send(update);
    (StatusCode::OK,Json(serde_json::json!({"status":"accepted","state":snapshot,"decision":decision,
        "v1_shared_market_vector":v1_vector_decision,"v2_relationship_discovery":v2_discovery,
        "market_vector":market_vector,"market_vector_persisted":should_evaluate_v1_now})))
}
async fn get_state(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let mut filter=doc!{"symbol":&symbol};if let Some(venue)=q.venue.as_deref(){filter.insert("venue",venue);}
    if let Ok(g)=s.memory.read(){
        let value=if let Some(venue)=q.venue.as_deref(){g.get(&format!("{venue}|{symbol}"))}
            else{g.values().filter(|value|value.symbol==symbol).max_by_key(|value|value.last_event_ts_ms.unwrap_or(0))};
        if let Some(value)=value{return(StatusCode::OK,Json(serde_json::to_value(value).unwrap_or_default()));}
    }
    match s.states.find_one(filter,FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut document))=>{document.remove("_id");document.remove("stored_at");
            (StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"event_count":0,"order_flow_imbalance":0.0,"state_quality":0.0}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"state storage unavailable"})))
    }
}
async fn get_vector(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    match s.decisions.find_one(doc!{"symbol":&symbol},FindOneOptions::builder().sort(doc!{"stored_at":-1}).build()).await{
        Ok(Some(mut d))=>{d.remove("_id");d.remove("stored_at");(StatusCode::OK,Json(mongodb::bson::from_document::<serde_json::Value>(d).unwrap_or_default()))},
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"action":"wait","reason":"no decision available"}))),
        Err(_)=>(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"decision storage unavailable"})))
    }
}
async fn get_latest_market_vector(State(s):State<AppState>,Query(q):Query<SymbolQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let mut filter=doc!{"symbol":&symbol};if let Some(venue)=q.venue.as_deref(){filter.insert("venue",venue);}
    match s.market_vectors.find_one(filter,FindOneOptions::builder().sort(doc!{"event_time_cutoff_ms":-1}).build()).await{
        Ok(Some(mut document))=>{
            document.remove("_id");
            match mongodb::bson::from_document::<MarketVector>(document){
                Ok(vector)=>(StatusCode::OK,Json(serde_json::to_value(vector).unwrap_or_default())),
                Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"stored market vector is invalid");
                    (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"stored market vector is invalid","symbol":symbol})))}
            }
        }
        Ok(None)=>(StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"status":"warming_up","message":"causal feature normalization requires at least eight prior observations"}))),
        Err(error)=>{tracing::error!(error=%error,symbol=%symbol,"market vector lookup failed");
            (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector storage unavailable"})))}
    }
}


#[derive(Deserialize)]
struct HistoryQuery { symbol: Option<String>, venue: Option<String>, limit: Option<i64> }

#[derive(Deserialize)]
struct BacktestQuery {
    symbol: Option<String>,
    venue: Option<String>,
    from_ts_ms: Option<i64>,
    to_ts_ms: Option<i64>,
    limit: Option<i64>,
    initial_capital: Option<f64>,
    fee_bps: Option<f64>,
    slippage_bps: Option<f64>,
    latency_ms: Option<i64>,
    max_participation_rate: Option<f64>,
    market_impact_bps: Option<f64>,
}
async fn run_backtest(State(s): State<AppState>, Query(q): Query<BacktestQuery>) -> impl IntoResponse {
    let symbol = q.symbol.unwrap_or_else(|| "BTCUSDT".to_string());
    if symbol.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"symbol is required"})));
    }
    let from_ts_ms = q.from_ts_ms.unwrap_or(0);
    let to_ts_ms = q.to_ts_ms.unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
    if from_ts_ms < 0 || to_ts_ms <= 0 || from_ts_ms > to_ts_ms {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"invalid replay time range"})));
    }
    let limit = q.limit.unwrap_or(50_000);
    if !(1..=50_000).contains(&limit) {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"limit must be between 1 and 50000"})));
    }
    let mut filter = doc! {"symbol": &symbol, "ts_ms": {"$gte": from_ts_ms, "$lte": to_ts_ms}};
    if let Some(venue) = q.venue.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        filter.insert("venue", venue);
    }
    let options = FindOptions::builder()
        .sort(doc! {"ts_ms": 1, "event_id": 1})
        .limit(limit + 1)
        .build();
    let mut cursor = match s.events.find(filter, Some(options)).await {
        Ok(cursor) => cursor,
        Err(error) => {
            tracing::error!(error=%error, symbol=%symbol, "backtest event query failed");
            return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"historical events unavailable"})));
        }
    };
    let mut events = Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document)) => {
                document.remove("_id");
                match mongodb::bson::from_document::<backtest::ReplayEvent>(document) {
                    Ok(event) => events.push(event),
                    Err(error) => {
                        tracing::error!(error=%error, symbol=%symbol, "stored event could not be decoded for replay");
                        return (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":"stored event schema is not replay-compatible"})));
                    }
                }
            }
            Ok(None) => break,
            Err(error) => {
                tracing::error!(error=%error, symbol=%symbol, "backtest event cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error":"historical event read failed"})));
            }
        }
    }
    if events.len() as i64 > limit {
        return (StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({
            "error":"replay window exceeds the event limit; narrow the timestamp range or select a venue",
            "limit":limit,
            "truncated":true
        })));
    }
    let strategy = match s.active_strategy.read() {
        Ok(strategy) => strategy.clone(),
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"strategy lock poisoned"}))),
    };
    match backtest::replay_with_execution(
        events,
        &strategy,
        q.initial_capital.unwrap_or(10_000.0),
        q.fee_bps.unwrap_or(10.0),
        q.slippage_bps.unwrap_or(5.0),
        q.latency_ms.unwrap_or(0),
        q.max_participation_rate.unwrap_or(0.10),
        q.market_impact_bps.unwrap_or(5.0),
    ) {
        Ok(result) => (StatusCode::OK, Json(serde_json::to_value(result).unwrap_or_default())),
        Err(message) => (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({"error":message}))),
    }
}

async fn spawn_binance_open_interest(axum_addr: String, symbols: Vec<String>) {
    if symbols.is_empty() { return; }
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    loop {
        for symbol in &symbols {
            let response=client.get("https://fapi.binance.com/fapi/v1/openInterest")
                .query(&[("symbol",symbol.as_str())]).send().await;
            let response=match response {
                Ok(value) if value.status().is_success()=>value,
                Ok(value)=>{tracing::warn!(symbol=%symbol,status=%value.status(),"Binance open-interest endpoint rejected request");continue;}
                Err(error)=>{tracing::warn!(symbol=%symbol,error=%error,"Binance open-interest request failed");continue;}
            };
            let payload:serde_json::Value=match response.json().await {
                Ok(value)=>value,Err(error)=>{tracing::warn!(symbol=%symbol,error=%error,"invalid Binance open-interest JSON");continue;}
            };
            let Some(open_interest)=stream_number(payload.get("openInterest")) else {
                tracing::warn!(symbol=%symbol,payload=?payload,"Binance open-interest response missing numeric value");
                continue;
            };
            let Some(ts_ms)=payload.get("time").and_then(|v|v.as_i64()) else {
                tracing::warn!(symbol=%symbol,"Binance open-interest response missing exchange timestamp");
                continue;
            };
            let event=MarketEvent {
                event_id:format!("open_interest:{ts_ms}"),venue:"binance_usdm".into(),symbol:symbol.clone(),
                event_type:"open_interest".into(),ts_ms,price:None,size:None,side:None,
                bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,
                open_interest:Some(open_interest),mark_price:None,index_price:None,liquidation_side:None,
            };
            submit_market_event(&client,&endpoint,&event,"binance_usdm_open_interest").await;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}
fn parse_kraken_trade_message(payload: &serde_json::Value) -> Vec<MarketEvent> {
    if payload.get("channel").and_then(|v|v.as_str()) != Some("trade") ||
       payload.get("type").and_then(|v|v.as_str()) != Some("update") {
        return Vec::new();
    }
    let Some(rows)=payload.get("data").and_then(|v|v.as_array()) else { return Vec::new(); };
    let mut parsed=Vec::new();
    for row in rows {
        let (Some(product),Some(side),Some(time))=(
            row.get("symbol").and_then(|v|v.as_str()),
            row.get("side").and_then(|v|v.as_str()),
            row.get("timestamp").and_then(|v|v.as_str()),
        ) else { continue; };
        let Some(timestamp)=chrono::DateTime::parse_from_rfc3339(time).ok().map(|v|v.timestamp_millis()) else { continue; };
        let side=match side.to_ascii_lowercase().as_str() {"buy"=>"buy","sell"=>"sell",_=>continue};
        let (Some(price),Some(size))=(stream_number(row.get("price")),stream_number(row.get("qty"))) else {continue;};
        let Some(trade_id)=row.get("trade_id").map(|v|v.to_string()) else {continue;};
        let observation=MarketEvent{
            event_id:format!("kraken_trade:{trade_id}"),
            venue:"kraken_spot".into(),
            symbol:format!("{}_KRAKEN",product.replace('/',"").replace('-',"").to_ascii_uppercase()),
            event_type:"trade".into(),ts_ms:timestamp,price:Some(price),size:Some(size),
            side:Some(side.into()),bid:None,ask:None,bid_size:None,ask_size:None,
            funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
        };
        if validate_event(&observation).is_ok(){parsed.push(observation);}
    }
    parsed
}

async fn spawn_kraken_stream(axum_addr:String,symbols:Vec<String>){
    if symbols.is_empty(){return;}
    let url="wss://ws.kraken.com/v2";
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    let mut backoff=1_u64;
    loop{
        match connect_async(url).await{
            Ok((mut socket,_))=>{
                tracing::info!(symbols=?symbols,"connected to Kraken Spot public market stream");
                backoff=1;
                let subscription=serde_json::json!({"method":"subscribe","params":{"channel":"trade","symbol":symbols}});
                if let Err(error)=socket.send(tokio_tungstenite::tungstenite::Message::Text(subscription.to_string())).await{
                    tracing::warn!(error=%error,"Kraken subscription send failed");
                }else{
                    while let Some(message)=socket.next().await{
                        let message=match message{Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"Kraken stream disconnected");break;}};
                        let text=match message.to_text(){Ok(v)=>v,Err(_)=>continue};
                        let payload:serde_json::Value=match serde_json::from_str(text){
                            Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"invalid Kraken stream JSON");continue;}
                        };
                        for event in parse_kraken_trade_message(&payload){
                            submit_market_event(&client,&endpoint,&event,"kraken_spot").await;
                        }
                    }
                }
            }
            Err(error)=>tracing::warn!(error=%error,retry_seconds=backoff,"Kraken stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff=(backoff*2).min(30);
    }
}

fn parse_coinbase_trade_message(payload: &serde_json::Value) -> Vec<MarketEvent> {
    if payload.get("channel").and_then(|v|v.as_str()) != Some("market_trades") {
        return Vec::new();
    }
    let Some(events) = payload.get("events").and_then(|v|v.as_array()) else {
        return Vec::new();
    };
    let mut parsed = Vec::new();
    for event in events {
        let Some(trades) = event.get("trades").and_then(|v|v.as_array()) else { continue; };
        for trade in trades {
            let (Some(trade_id), Some(product), Some(side), Some(time)) = (
                trade.get("trade_id").and_then(|v|v.as_str()),
                trade.get("product_id").and_then(|v|v.as_str()),
                trade.get("side").and_then(|v|v.as_str()),
                trade.get("time").and_then(|v|v.as_str()),
            ) else { continue; };
            let Some(timestamp) = chrono::DateTime::parse_from_rfc3339(time).ok()
                .map(|value| value.timestamp_millis()) else { continue; };
            let symbol = format!("{}_COINBASE", product.replace('-', "").replace('/', "").to_ascii_uppercase());
            let side = match side.to_ascii_uppercase().as_str() {
                "BUY" => "buy",
                "SELL" => "sell",
                _ => continue,
            };
            let (Some(price), Some(size)) = (
                stream_number(trade.get("price")),
                stream_number(trade.get("size")),
            ) else { continue; };
            let observation = MarketEvent {
                event_id: format!("coinbase_trade:{trade_id}"),
                venue: "coinbase_advanced_trade".into(),
                symbol,
                event_type: "trade".into(),
                ts_ms: timestamp,
                price: Some(price),
                size: Some(size),
                side: Some(side.into()),
                bid: None, ask: None, bid_size: None, ask_size: None,
                funding_rate: None, open_interest: None,mark_price:None,index_price:None, liquidation_side: None,
            };
            if validate_event(&observation).is_ok() {
                parsed.push(observation);
            }
        }
    }
    parsed
}

async fn spawn_coinbase_stream(axum_addr: String, products: Vec<String>) {
    if products.is_empty() { return; }
    let url = "wss://advanced-trade-ws.coinbase.com";
    let client = reqwest::Client::new();
    let endpoint = format!("http://{axum_addr}/internal/events");
    let mut backoff = 1_u64;
    loop {
        match connect_async(url).await {
            Ok((mut socket, _)) => {
                tracing::info!(products=?products, "connected to Coinbase Advanced Trade public market stream");
                backoff = 1;
                let subscription = serde_json::json!({
                    "type":"subscribe",
                    "channel":"market_trades",
                    "product_ids":products
                });
                if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Text(subscription.to_string())).await {
                    tracing::warn!(error=%error, "Coinbase subscription send failed");
                } else {
                    while let Some(message) = socket.next().await {
                        let message = match message {
                            Ok(value) => value,
                            Err(error) => { tracing::warn!(error=%error, "Coinbase stream disconnected"); break; }
                        };
                        if message.is_ping() {
                            if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Pong(message.into_data())).await {
                                tracing::warn!(error=%error, "Coinbase websocket pong failed");
                                break;
                            }
                            continue;
                        }
                        let text = match message.to_text() { Ok(value) => value, Err(_) => continue };
                        let payload: serde_json::Value = match serde_json::from_str(text) {
                            Ok(value) => value,
                            Err(error) => { tracing::warn!(error=%error, "invalid Coinbase market stream JSON"); continue; }
                        };
                        for event in parse_coinbase_trade_message(&payload) {
                            submit_market_event(&client, &endpoint, &event, "coinbase_advanced_trade").await;
                        }
                    }
                }
            }
            Err(error) => tracing::warn!(error=%error,retry_seconds=backoff,"Coinbase stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}

async fn spawn_finnhub_stream(axum_addr: String, token: String, symbols: Vec<String>) {
    if symbols.is_empty() { return; }
    let url = format!("wss://ws.finnhub.io?token={token}");
    let client = reqwest::Client::new();
    let endpoint = format!("http://{axum_addr}/internal/events");
    let mut backoff = 1_u64;
    loop {
        match connect_async(url.as_str()).await {
            Ok((mut socket, _)) => {
                tracing::info!(symbols=?symbols, "connected to Finnhub public market trade stream");
                backoff = 1;
                for symbol in &symbols {
                    let command = serde_json::json!({"type":"subscribe","symbol":symbol});
                    if let Err(error) = socket.send(tokio_tungstenite::tungstenite::Message::Text(command.to_string())).await {
                        tracing::warn!(symbol=%symbol,error=%error,"Finnhub subscription send failed");
                        break;
                    }
                }
                while let Some(message) = socket.next().await {
                    let message = match message {
                        Ok(value) => value,
                        Err(error) => { tracing::warn!(error=%error,"Finnhub stream disconnected"); break; }
                    };
                    let text = match message.to_text() { Ok(value) => value, Err(_) => continue };
                    let payload: serde_json::Value = match serde_json::from_str(text) {
                        Ok(value) => value,
                        Err(error) => { tracing::warn!(error=%error,"invalid Finnhub stream JSON"); continue; }
                    };
                    let message_type=payload.get("type").and_then(|v|v.as_str()).unwrap_or_default();
                    if message_type=="error" {
                        tracing::warn!(provider_message=?payload.get("msg"),"Finnhub reported a subscription or entitlement error");
                        continue;
                    }
                    if message_type != "trade" { continue; }
                    let Some(rows) = payload.get("data").and_then(|v|v.as_array()) else { continue; };
                    for (row_index, row) in rows.iter().enumerate() {
                        let Some(symbol) = row.get("s").and_then(|v|v.as_str()) else { continue; };
                        let Some(ts_ms) = row.get("t").and_then(|v|v.as_i64()) else {
                            tracing::warn!(symbol=%symbol,"Finnhub trade missing provider timestamp; observation skipped");
                            continue;
                        };
                        let price = stream_number(row.get("p"));
                        let size = stream_number(row.get("v"));
                        let event_id = format!("trade:{}:{}:{}:{}",ts_ms,price.unwrap_or_default(),size.unwrap_or_default(),row_index);
                        let event = MarketEvent {
                            event_id, venue:"finnhub".into(), symbol:symbol.into(), event_type:"trade".into(), ts_ms,
                            price, size, side:None, bid:None, ask:None, bid_size:None, ask_size:None,
                            funding_rate:None, open_interest:None,mark_price:None,index_price:None, liquidation_side:None,
                        };
                        submit_market_event(&client,&endpoint,&event,"finnhub").await;
                    }
                }
            }
            Err(error) => tracing::warn!(error=%error,retry_seconds=backoff,"Finnhub market stream connection failed"),
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff = (backoff * 2).min(30);
    }
}
async fn get_events(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(100).clamp(1,1000);
    let options=FindOptions::builder().sort(doc!{"ts_ms":-1}).limit(limit).build();
    let mut cursor=match s.events.find(doc!{"symbol":&symbol},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"raw event history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"raw event history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{document.remove("_id");items.push(mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default());}
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"raw event history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"raw event history read failed","symbol":symbol}))); }
        }
    }
    items.reverse();
    (StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"items":items,"count":items.len(),"limit":limit})))
}
async fn get_vector_history(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse {
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(120).clamp(1,500);
    let options=FindOptions::builder().sort(doc!{"stored_at":-1}).limit(limit).build();
    let mut cursor=match s.decisions.find(doc!{"symbol":&symbol},Some(options)).await {
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"vector history query failed");
            return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop {
        match cursor.try_next().await {
            Ok(Some(mut document))=>{
                let stored_at=document.get_datetime("stored_at").ok().map(|value|chrono::DateTime::<chrono::Utc>::from_timestamp_millis(value.timestamp_millis()).map(|v|v.to_rfc3339()).unwrap_or_default());
                document.remove("_id"); document.remove("stored_at");
                let mut value= mongodb::bson::from_document::<serde_json::Value>(document).unwrap_or_default();
                if let (Some(object),Some(timestamp))=(value.as_object_mut(),stored_at){object.insert("stored_at".into(),serde_json::Value::String(timestamp));}
                items.push(value);
            }
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"vector history cursor failed");
                return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector history read failed","symbol":symbol}))); }
        }
    }
    items.reverse();
    (StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"items":items,"count":items.len(),"limit":limit})))
}
async fn get_market_vector_history(State(s):State<AppState>,Query(q):Query<HistoryQuery>)->impl IntoResponse{
    let symbol=q.symbol.unwrap_or_else(||"BTCUSDT".into());
    let limit=q.limit.unwrap_or(120).clamp(1,500);
    let mut filter=doc!{"symbol":&symbol};if let Some(venue)=q.venue.as_deref(){filter.insert("venue",venue);}
    let options=FindOptions::builder().sort(doc!{"event_time_cutoff_ms":-1}).limit(limit).build();
    let mut cursor=match s.market_vectors.find(filter,Some(options)).await{
        Ok(cursor)=>cursor,
        Err(error)=>{tracing::error!(error=%error,"market vector history query failed");
            return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector history unavailable","symbol":symbol}))); }
    };
    let mut items=Vec::new();
    loop{
        match cursor.try_next().await{
            Ok(Some(mut document))=>{
                document.remove("_id");document.remove("stored_at");
                match mongodb::bson::from_document::<MarketVector>(document){
                    Ok(vector)=>items.push(serde_json::to_value(vector).unwrap_or_default()),
                    Err(error)=>{tracing::error!(error=%error,"invalid market vector in history");
                        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector history contains invalid data"}))); }
                }
            }
            Ok(None)=>break,
            Err(error)=>{tracing::error!(error=%error,"market vector history cursor failed");
                return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector history read failed","symbol":symbol}))); }
        }
    }
    items.reverse();
    (StatusCode::OK,Json(serde_json::json!({"symbol":symbol,"items":items,"count":items.len(),"limit":limit})))
}

async fn get_instruments()->impl IntoResponse {
    let finnhub_configured=env::var("FINNHUB_API_KEY").map(|v|!v.trim().is_empty()).unwrap_or(false);
    let binance_enabled=env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true);
    let mut items=serde_json::json!([
      {"symbol":"BTC/USD","engine_symbol":"BTCUSDT","name":"Bitcoin","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Bitcoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSDT","name":"Ethereum","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Ether perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"BTC/USD","engine_symbol":"BTCUSD_COINBASE","name":"Bitcoin","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Bitcoin spot · USD quote","quote_currency":"USD"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSD_COINBASE","name":"Ethereum","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Ether spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSD_COINBASE","name":"Solana","asset_class":"Crypto","venue":"Coinbase Advanced Trade","venue_id":"coinbase_advanced_trade","provider":"Coinbase public market stream","description":"Solana spot · USD quote","quote_currency":"USD"},
      {"symbol":"BTC/USD","engine_symbol":"BTCUSD_KRAKEN","name":"Bitcoin","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Bitcoin spot · USD quote","quote_currency":"USD"},
      {"symbol":"ETH/USD","engine_symbol":"ETHUSD_KRAKEN","name":"Ethereum","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Ether spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSD_KRAKEN","name":"Solana","asset_class":"Crypto","venue":"Kraken Spot","venue_id":"kraken_spot","provider":"Kraken public market stream","description":"Solana spot · USD quote","quote_currency":"USD"},
      {"symbol":"SOL/USD","engine_symbol":"SOLUSDT","name":"Solana","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Solana perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"BNB/USD","engine_symbol":"BNBUSDT","name":"BNB","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"BNB perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"XRP/USD","engine_symbol":"XRPUSDT","name":"XRP","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"XRP perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"DOGE/USD","engine_symbol":"DOGEUSDT","name":"Dogecoin","asset_class":"Crypto","venue":"Binance USD-M","venue_id":"binance_usdm","provider":"Binance public market stream","description":"Dogecoin perpetual · USDT quote","quote_currency":"USDT"},
      {"symbol":"AAPL","engine_symbol":"AAPL","name":"Apple","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Apple Inc. · common stock","quote_currency":"USD"},
      {"symbol":"MSFT","engine_symbol":"MSFT","name":"Microsoft","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Microsoft Corporation · common stock","quote_currency":"USD"},
      {"symbol":"NVDA","engine_symbol":"NVDA","name":"NVIDIA","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"NVIDIA Corporation · common stock","quote_currency":"USD"},
      {"symbol":"AMZN","engine_symbol":"AMZN","name":"Amazon","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Amazon.com Inc. · common stock","quote_currency":"USD"},
      {"symbol":"GOOGL","engine_symbol":"GOOGL","name":"Alphabet","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Alphabet Inc. · Class A","quote_currency":"USD"},
      {"symbol":"META","engine_symbol":"META","name":"Meta Platforms","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Meta Platforms Inc. · common stock","quote_currency":"USD"},
      {"symbol":"TSLA","engine_symbol":"TSLA","name":"Tesla","asset_class":"Equities","venue":"NASDAQ","venue_id":"finnhub","provider":"Finnhub","description":"Tesla Inc. · common stock","quote_currency":"USD"},
      {"symbol":"EUR/USD","engine_symbol":"OANDA:EUR_USD","name":"Euro / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"GBP/USD","engine_symbol":"OANDA:GBP_USD","name":"Pound / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"USD/JPY","engine_symbol":"OANDA:USD_JPY","name":"US Dollar / Yen","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"JPY"},
      {"symbol":"AUD/USD","engine_symbol":"OANDA:AUD_USD","name":"Australian Dollar / US Dollar","asset_class":"FX","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Major FX pair","quote_currency":"USD"},
      {"symbol":"SPX500","engine_symbol":"OANDA:SPX500_USD","name":"S&P 500","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US large-cap equity index CFD reference","quote_currency":"USD"},
      {"symbol":"NAS100","engine_symbol":"OANDA:NAS100_USD","name":"NASDAQ 100","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US technology-heavy equity index reference","quote_currency":"USD"},
      {"symbol":"US30","engine_symbol":"OANDA:US30_USD","name":"Dow Jones 30","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US blue-chip equity index reference","quote_currency":"USD"},
      {"symbol":"GER40","engine_symbol":"OANDA:DE30_EUR","name":"DAX 40","asset_class":"Indices","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"German equity index reference","quote_currency":"EUR"},
      {"symbol":"XAU/USD","engine_symbol":"OANDA:XAU_USD","name":"Gold","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Gold spot reference","quote_currency":"USD"},
      {"symbol":"XAG/USD","engine_symbol":"OANDA:XAG_USD","name":"Silver","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Silver spot reference","quote_currency":"USD"},
      {"symbol":"WTI","engine_symbol":"OANDA:WTICO_USD","name":"WTI Crude Oil","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"US crude oil reference","quote_currency":"USD"},
      {"symbol":"BRENT","engine_symbol":"OANDA:BCO_USD","name":"Brent Crude","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Brent crude oil reference","quote_currency":"USD"},
      {"symbol":"NATGAS","engine_symbol":"OANDA:NATGAS_USD","name":"Natural Gas","asset_class":"Commodities","venue":"OANDA","venue_id":"finnhub","provider":"Finnhub","description":"Natural gas reference","quote_currency":"USD"}
    ]);
    if let Some(rows)=items.as_array_mut() {
        for row in rows {
            let venue_id=row.get("venue_id").and_then(|v|v.as_str()).unwrap_or_default();
            if venue_id=="coinbase_advanced_trade" {
                let enabled=env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
                row["provider_status"]=serde_json::json!(if enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else if venue_id=="kraken_spot" {
                let enabled=env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
                row["provider_status"]=serde_json::json!(if enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else if venue_id=="binance_usdm" {
                row["provider_status"]=serde_json::json!(if binance_enabled {"public_stream_enabled"} else {"stream_disabled"});
            } else {
                row["provider_status"]=serde_json::json!(if finnhub_configured {"provider_key_configured"} else {"requires_FINNHUB_API_KEY"});
            }
        }
    }
    (StatusCode::OK,Json(serde_json::json!({"source":"Qoin instrument registry","items":items,"count":items.as_array().map(|v|v.len()).unwrap_or(0)})))
}
#[derive(Deserialize)]
struct NewsQuery { category: Option<String> }
async fn get_calendar()->impl IntoResponse {
    let key=env::var("FINNHUB_API_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "source":"Finnhub","status":"not_configured","items":[],
            "message":"Set FINNHUB_API_KEY to load sourced economic-calendar events."
        })));
    }
    let from=chrono::Utc::now().date_naive();
    let to=from+chrono::Duration::days(7);
    let from_text=from.format("%Y-%m-%d").to_string();
    let to_text=to.format("%Y-%m-%d").to_string();
    let response=match reqwest::Client::new().get("https://finnhub.io/api/v1/calendar/economic")
        .query(&[("from",from_text.as_str()),("to",to_text.as_str()),("token",key.as_str())]).send().await {
        Ok(value)=>value,
        Err(error)=>{tracing::warn!(error=%error,"Finnhub economic calendar request failed");
            return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_unavailable","items":[]}))); }
    };
    if !response.status().is_success() {
        tracing::warn!(status=%response.status(),"Finnhub calendar provider returned non-success");
        return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_error","items":[]})));
    }
    let payload:serde_json::Value=match response.json().await {
        Ok(value)=>value,
        Err(_)=>return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"invalid_provider_response","items":[]})))
    };
    let items=payload.get("economicCalendar").and_then(|v|v.get("events")).and_then(|v|v.as_array())
        .cloned().unwrap_or_default().into_iter().map(|event|serde_json::json!({
            "country":event.get("country").and_then(|v|v.as_str()).unwrap_or(""),
            "event":event.get("event").and_then(|v|v.as_str()).unwrap_or(""),
            "impact":event.get("impact").and_then(|v|v.as_str()).unwrap_or(""),
            "actual":event.get("actual").cloned().unwrap_or(serde_json::Value::Null),
            "previous":event.get("prev").cloned().unwrap_or(serde_json::Value::Null),
            "estimate":event.get("estimate").cloned().unwrap_or(serde_json::Value::Null),
            "time":event.get("time").and_then(|v|v.as_str()).unwrap_or(""),
            "unit":event.get("unit").and_then(|v|v.as_str()).unwrap_or("")
        })).collect::<Vec<_>>();
    (StatusCode::OK,Json(serde_json::json!({"source":"Finnhub economic calendar","status":"ok","from":from_text,"to":to_text,"items":items,"count":items.len()})))
}
async fn get_news(Query(query):Query<NewsQuery>)->impl IntoResponse {
    let key=env::var("FINNHUB_API_KEY").unwrap_or_default();
    if key.trim().is_empty() {
        return (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "source":"Finnhub","status":"not_configured","items":[],
            "message":"Set FINNHUB_API_KEY to load sourced live headlines. No placeholder headlines are generated."
        })));
    }
    let category=query.category.unwrap_or_else(||"general".into());
    if !["general","forex","crypto","merger"].contains(&category.as_str()) {
        return (StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"category must be general, forex, crypto, or merger"})));
    }
    let response=match reqwest::Client::new().get("https://finnhub.io/api/v1/news")
        .query(&[("category",category.as_str()),("token",key.as_str())]).send().await {
        Ok(value)=>value,
        Err(error)=>{tracing::warn!(error=%error,"Finnhub news request failed");
            return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_unavailable","items":[],"message":"News provider request failed."}))); }
    };
    if !response.status().is_success() {
        let status=response.status();
        tracing::warn!(status=%status,"Finnhub news provider returned non-success");
        return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"provider_error","items":[],"message":"News provider rejected the request."})));
    }
    let payload:serde_json::Value=match response.json().await {
        Ok(value)=>value,
        Err(_)=>return (StatusCode::BAD_GATEWAY,Json(serde_json::json!({"source":"Finnhub","status":"invalid_provider_response","items":[]})))
    };
    let items=payload.as_array().cloned().unwrap_or_default().into_iter().filter_map(|item|{
        let headline=item.get("headline")?.as_str()?.trim();
        let url=item.get("url")?.as_str()?.trim();
        if headline.is_empty()||!url.starts_with("https://"){return None;}
        Some(serde_json::json!({
            "id":item.get("id").cloned().unwrap_or(serde_json::Value::Null),
            "headline":headline,"summary":item.get("summary").and_then(|v|v.as_str()).unwrap_or(""),
            "source":item.get("source").and_then(|v|v.as_str()).unwrap_or("Finnhub"),
            "url":url,"image":item.get("image").and_then(|v|v.as_str()).unwrap_or(""),
            "published_at_ms":item.get("datetime").and_then(|v|v.as_i64()).unwrap_or(0)*1000,
            "related":item.get("related").and_then(|v|v.as_str()).unwrap_or(""),
            "category":item.get("category").and_then(|v|v.as_str()).unwrap_or(category.as_str())
        }))
    }).collect::<Vec<_>>();
    (StatusCode::OK,Json(serde_json::json!({"source":"Finnhub","status":"ok","items":items,"count":items.len()})))
}

fn subscription_set(symbol: Option<String>, symbols: Option<String>) -> HashSet<String> {
    symbols.or(symbol).unwrap_or_default().split(',').map(str::trim)
        .filter(|v| !v.is_empty()).map(|v| v.to_ascii_uppercase()).collect()
}
async fn stream_ws(State(state): State<AppState>, Query(query): Query<StreamQuery>, ws: WebSocketUpgrade) -> impl IntoResponse {
    let symbols = subscription_set(query.symbol, query.symbols);
    ws.on_upgrade(move |socket| stream_connection(socket, state, symbols))
}
async fn stream_connection(mut socket: WebSocket, state: AppState, mut symbols: HashSet<String>) {
    let hello = serde_json::json!({"type":"stream.ready","protocol_version":1,
        "subscribed_symbols":symbols,"server_ts_ms":chrono::Utc::now().timestamp_millis(),
        "note":"Read-only market observations and research vectors; no order execution."});
    if socket.send(Message::Text(hello.to_string())).await.is_err() { return; }
    let mut updates = state.stream.subscribe();
    loop {
        tokio::select! {
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(command) = serde_json::from_str::<serde_json::Value>(&text) {
                        if command.get("type").and_then(|v|v.as_str()) == Some("subscribe") {
                            let requested = command.get("symbols").and_then(|v|v.as_array())
                                .map(|items| items.iter().filter_map(|v|v.as_str())
                                    .map(|v|v.trim().to_ascii_uppercase()).filter(|v|!v.is_empty()).collect::<HashSet<_>>())
                                .or_else(||command.get("symbol").and_then(|v|v.as_str())
                                    .map(|v|[v.trim().to_ascii_uppercase()].into_iter().collect()));
                            if let Some(next)=requested { symbols=next;
                                let ack=serde_json::json!({"type":"stream.subscribed","subscribed_symbols":symbols,
                                    "server_ts_ms":chrono::Utc::now().timestamp_millis()});
                                if socket.send(Message::Text(ack.to_string())).await.is_err(){break;}
                            }
                        }
                    }
                }
                Some(Ok(Message::Ping(payload))) => if socket.send(Message::Pong(payload)).await.is_err(){break;},
                Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                _ => {}
            },
            update = updates.recv() => match update {
                Ok(value) => {
                    let symbol=value.get("symbol").and_then(|v|v.as_str()).unwrap_or_default().to_ascii_uppercase();
                    if !symbols.is_empty() && !symbols.contains(&symbol) {continue;}
                    if socket.send(Message::Text(value.to_string())).await.is_err(){break;}
                }
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    let gap=serde_json::json!({"type":"stream.gap","missed_updates":missed,
                        "server_ts_ms":chrono::Utc::now().timestamp_millis(),
                        "note":"Backpressure dropped updates. Refresh REST snapshots before trusting the live view."});
                    if socket.send(Message::Text(gap.to_string())).await.is_err(){break;}
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
fn stream_number(value: Option<&serde_json::Value>) -> Option<f64> {
    value.and_then(|v|v.as_f64().or_else(||v.as_str().and_then(|s|s.parse::<f64>().ok())))
}
async fn submit_market_event(client: &reqwest::Client, endpoint: &str, event: &MarketEvent, provider: &str) {
    match client.post(endpoint).json(event).send().await {
        Ok(response) if !response.status().is_success() => {
            let status=response.status();
            let body=response.text().await.unwrap_or_default();
            tracing::warn!(provider=%provider,symbol=%event.symbol,event_id=%event.event_id,status=%status,
                response=%body.chars().take(300).collect::<String>(),"provider observation rejected by engine ingestion");
        }
        Ok(_) => {}
        Err(error) => tracing::warn!(provider=%provider,symbol=%event.symbol,event_id=%event.event_id,
            error=%error,"provider observation could not reach engine ingestion"),
    }
}
async fn spawn_binance_stream(axum_addr: String, symbols: Vec<String>) {
    if symbols.is_empty(){return;}
    let streams=symbols.iter().flat_map(|symbol|{
        let symbol=symbol.to_ascii_lowercase();
        [format!("{symbol}@trade"),format!("{symbol}@bookTicker"),
         format!("{symbol}@markPrice@1s"),format!("{symbol}@forceOrder")]
    }).collect::<Vec<_>>().join("/");
    let url=format!("wss://fstream.binance.com/stream?streams={streams}");
    let client=reqwest::Client::new();
    let endpoint=format!("http://{axum_addr}/internal/events");
    let mut backoff=1_u64;
    loop {
        match connect_async(&url).await {
            Ok((mut socket,_)) => {
                tracing::info!(symbols=?symbols,"connected to Binance USD-M public market streams");
                backoff=1;
                while let Some(message)=socket.next().await {
                    let message=match message {Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"Binance stream disconnected");break;}};
                    let text=match message.to_text(){Ok(v)=>v,Err(_)=>continue};
                    let envelope:serde_json::Value=match serde_json::from_str(text){Ok(v)=>v,Err(error)=>{tracing::warn!(error=%error,"invalid Binance stream JSON");continue;}};
                    let data=envelope.get("data").unwrap_or(&envelope);
                    let event_name=data.get("e").and_then(|v|v.as_str()).unwrap_or_default();
                    let symbol=data.get("s").and_then(|v|v.as_str()).unwrap_or_default();
                    if symbol.is_empty(){continue;}
                    let Some(timestamp)=data.get("E").and_then(|v|v.as_i64()) else { continue; };
                    let event=if event_name=="trade" {
                        let id=data.get("t").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string());
                        Some(MarketEvent{event_id:format!("trade:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"trade".into(),ts_ms:timestamp,price:stream_number(data.get("p")),size:stream_number(data.get("q")),
                            side:Some(if data.get("m").and_then(|v|v.as_bool()).unwrap_or(false){"sell"}else{"buy"}.into()),
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None})
                    } else if data.get("b").is_some()&&data.get("a").is_some()&&data.get("u").is_some() {
                        let id=data.get("u").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string());
                        Some(MarketEvent{event_id:format!("book:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"book".into(),ts_ms:timestamp,price:None,size:None,side:None,
                            bid:stream_number(data.get("b")),ask:stream_number(data.get("a")),
                            bid_size:stream_number(data.get("B")),ask_size:stream_number(data.get("A")),
                            funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None})
                    } else if event_name=="markPriceUpdate" {
                        Some(MarketEvent{event_id:format!("funding:{timestamp}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"funding".into(),ts_ms:timestamp,price:None,size:None,side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:stream_number(data.get("r")),
                            open_interest:None,mark_price:stream_number(data.get("p")),index_price:stream_number(data.get("i")),
                            liquidation_side:None})
                    } else if event_name=="forceOrder" {
                        let order=data.get("o").unwrap_or(data);
                        let side=order.get("S").and_then(|v|v.as_str()).unwrap_or_default();
                        let liquidation_side=match side{"SELL"=>Some("long"),"BUY"=>Some("short"),_=>None};
                        let id=format!("{}:{}:{}:{}",order.get("T").map(|v|v.to_string()).unwrap_or_else(||timestamp.to_string()),side,order.get("p").and_then(|v|v.as_str()).unwrap_or(""),order.get("q").and_then(|v|v.as_str()).unwrap_or(""));
                        Some(MarketEvent{event_id:format!("liquidation:{id}"),venue:"binance_usdm".into(),symbol:symbol.into(),
                            event_type:"liquidation".into(),ts_ms:order.get("T").and_then(|v|v.as_i64()).unwrap_or(timestamp),
                            price:stream_number(order.get("p")),size:stream_number(order.get("q")),side:None,
                            bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,open_interest:None,mark_price:None,index_price:None,
                            liquidation_side:liquidation_side.map(str::to_owned)})
                    } else {None};
                    if let Some(event)=event {
                        submit_market_event(&client,&endpoint,&event,"binance_usdm").await;
                    }
                }
            }
            Err(error)=>tracing::warn!(error=%error,retry_seconds=backoff,"Binance stream connection failed")
        }
        tokio::time::sleep(Duration::from_secs(backoff)).await;
        backoff=(backoff*2).min(30);
    }
}

async fn get_strategy(State(s):State<AppState>)->impl IntoResponse{
    match s.active_strategy.read(){Ok(v)=>(StatusCode::OK,Json(serde_json::to_value(&*v).unwrap_or_default())),
        Err(_)=>(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned"})))}
}
async fn put_strategy(State(s):State<AppState>,Json(strategy):Json<EdgeStrategy>)->impl IntoResponse{
    if let Err(message)=strategy.validate(){return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":message})));}
    let mut stored=strategy.clone();
    stored.updated_at_ms=chrono::Utc::now().timestamp_millis();
    let doc=match mongodb::bson::to_document(&stored){Ok(v)=>v,Err(_)=>return(StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"strategy serialization failed"})))};
    let options=UpdateOptions::builder().upsert(true).build();
    if let Err(e)=s.strategies.update_one(doc!{"strategy_key":STRATEGY_KEY},doc!{"$set":doc},Some(options)).await{
        tracing::error!(error=%e,"strategy map persistence failed");
        return(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"strategy map persistence failed"})));
    }
    match s.active_strategy.write(){Ok(mut current)=>*current=stored.clone(),Err(_)=>return(StatusCode::INTERNAL_SERVER_ERROR,Json(serde_json::json!({"error":"strategy lock poisoned after persistence"})))}
    (StatusCode::OK,Json(serde_json::json!({"status":"strategy_updated","strategy":stored,"live_trading":false})))
}

#[derive(Deserialize)]
struct MarketVectorSignalRequest {
    vector: MarketVector,
    has_open_position: bool,
}

#[derive(Deserialize)]
struct VariableHubIdentityRequest { venue:String, symbol:String, timeframe:String }
#[derive(Deserialize)]
struct ObserveVariableRequest {
    venue:String, symbol:String, timeframe:String, feature:MarketFeature,
    raw_value:f64, quality:f64, event_time_ms:i64, available_at_ms:i64,
    producer:String, calculation_version:String,
}
#[derive(Deserialize)]
struct NormalizeVariableRequest {
    venue:String, symbol:String, timeframe:String, feature:MarketFeature,
    profile:NormalizationProfile,
}
#[derive(Deserialize)]
struct DeriveVariableRequest {
    venue:String, symbol:String, timeframe:String, feature:MarketFeature,
    raw_value:f64, quality:f64, event_time_ms:i64, available_at_ms:i64,
    producer:String, calculation_version:String,
    #[serde(default)] attribution:Option<AttributionReport>,
}
#[derive(Deserialize)]
struct MarkVariableNotApplicableRequest {
    venue:String, symbol:String, timeframe:String, feature:MarketFeature,
    event_time_ms:i64, available_at_ms:i64, producer:String, calculation_version:String,
}
#[derive(Deserialize)]
struct PublishVariableHubRequest {
    venue:String, symbol:String, timeframe:String, vector_id:String,
    event_time_cutoff_ms:i64, available_at_ms:i64,
}
#[derive(Deserialize)]
struct VariableHubQuery { venue:String, symbol:String, timeframe:String }

fn variable_hub_lock_index(state:&AppState,venue:&str,symbol:&str,timeframe:&str)->usize{
    let key=format!("{venue}|{symbol}|{timeframe}");
    key.bytes().fold(0usize,|hash,byte|hash.wrapping_mul(31).wrapping_add(byte as usize))
        % state.variable_hub_locks.len()
}
async fn load_or_create_variable_hub(
    state:&AppState, venue:&str, symbol:&str, timeframe:&str,
)->Result<VariableHub,String>{
    let filter=doc!{"identity.venue":venue,"identity.symbol":symbol,"identity.timeframe":timeframe};
    match state.variable_hubs.find_one(filter,None).await.map_err(|error|error.to_string())?{
        Some(mut document)=>{
            document.remove("_id");
            mongodb::bson::from_document::<VariableHub>(document).map_err(|error|error.to_string())
        }
        None=>VariableHub::new(venue,symbol,timeframe).map_err(|error|error.to_string()),
    }
}
async fn persist_variable_hub(state:&AppState,hub:&VariableHub)->Result<(),String>{
    let document=mongodb::bson::to_document(hub).map_err(|error|error.to_string())?;
    let filter=doc!{"identity.venue":&hub.identity.venue,"identity.symbol":&hub.identity.symbol,
        "identity.timeframe":&hub.identity.timeframe};
    state.variable_hubs.replace_one(filter,document,
        mongodb::options::ReplaceOptions::builder().upsert(true).build()).await
        .map(|_|()).map_err(|error|error.to_string())
}
fn variable_hub_error(error:HubError)->(StatusCode,Json<serde_json::Value>){
    (StatusCode::UNPROCESSABLE_ENTITY,Json(serde_json::json!({"error":error.to_string()})))
}
async fn observe_variable(
    State(state):State<AppState>,Json(request):Json<ObserveVariableRequest>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let lock_index=variable_hub_lock_index(&state,&request.venue,&request.symbol,&request.timeframe);
    let _hub_guard=state.variable_hub_locks[lock_index].lock().await;

    if !timestamps_are_not_future(request.event_time_ms,request.available_at_ms){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future variable timestamps are rejected"}))));
    }
    let mut hub=load_or_create_variable_hub(&state,&request.venue,&request.symbol,&request.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    let revision=hub.observe_input(request.feature,request.raw_value,request.quality,request.event_time_ms,
        request.available_at_ms,request.producer,request.calculation_version).map_err(variable_hub_error)?;
    persist_variable_hub(&state,&hub).await.map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error":"variable hub persistence failed","detail":error}))))?;
    Ok(Json(serde_json::json!({"revision":revision,"progress":hub.progress(request.feature)})))
}
async fn normalize_variable(
    State(state):State<AppState>,Json(request):Json<NormalizeVariableRequest>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let lock_index=variable_hub_lock_index(&state,&request.venue,&request.symbol,&request.timeframe);
    let _hub_guard=state.variable_hub_locks[lock_index].lock().await;

    let mut hub=load_or_create_variable_hub(&state,&request.venue,&request.symbol,&request.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    if hub.value(request.feature).is_some_and(|value|!timestamps_are_not_future(value.event_time_ms,value.available_at_ms)){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future variable observations cannot be normalized"}))));
    }
    let revision=hub.normalize(request.feature,request.profile).map_err(variable_hub_error)?;
    persist_variable_hub(&state,&hub).await.map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error":"variable hub persistence failed","detail":error}))))?;
    Ok(Json(serde_json::json!({"revision":revision,"progress":hub.progress(request.feature)})))
}
async fn derive_variable(
    State(state):State<AppState>,Json(request):Json<DeriveVariableRequest>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let lock_index=variable_hub_lock_index(&state,&request.venue,&request.symbol,&request.timeframe);
    let _hub_guard=state.variable_hub_locks[lock_index].lock().await;

    if !timestamps_are_not_future(request.event_time_ms,request.available_at_ms){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future derived-variable timestamps are rejected"}))));
    }
    let mut hub=load_or_create_variable_hub(&state,&request.venue,&request.symbol,&request.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    let revision=if let Some(attribution)=request.attribution{
        hub.record_derived_with_attribution(request.feature,request.raw_value,request.quality,
            request.event_time_ms,request.available_at_ms,request.producer,request.calculation_version,attribution)
    }else{
        hub.record_derived(request.feature,request.raw_value,request.quality,
            request.event_time_ms,request.available_at_ms,request.producer,request.calculation_version)
    }.map_err(variable_hub_error)?;
    persist_variable_hub(&state,&hub).await.map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error":"variable hub persistence failed","detail":error}))))?;
    Ok(Json(serde_json::json!({"revision":revision,"progress":hub.progress(request.feature)})))
}
async fn mark_variable_not_applicable(
    State(state):State<AppState>,Json(request):Json<MarkVariableNotApplicableRequest>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let lock_index=variable_hub_lock_index(&state,&request.venue,&request.symbol,&request.timeframe);
    let _hub_guard=state.variable_hub_locks[lock_index].lock().await;

    if !timestamps_are_not_future(request.event_time_ms,request.available_at_ms){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future not-applicable timestamps are rejected"}))));
    }
    let mut hub=load_or_create_variable_hub(&state,&request.venue,&request.symbol,&request.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    let revision=hub.mark_not_applicable(request.feature,request.event_time_ms,request.available_at_ms,
        request.producer,request.calculation_version).map_err(variable_hub_error)?;
    persist_variable_hub(&state,&hub).await.map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error":"variable hub persistence failed","detail":error}))))?;
    Ok(Json(serde_json::json!({"revision":revision,"progress":hub.progress(request.feature)})))
}
async fn get_variable_progress(
    State(state):State<AppState>,Query(query):Query<VariableHubQuery>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let hub=load_or_create_variable_hub(&state,&query.venue,&query.symbol,&query.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    Ok(Json(serde_json::json!({"identity":hub.identity,"variables":hub.progress_all()})))
}
async fn publish_variable_hub(
    State(state):State<AppState>,Json(request):Json<PublishVariableHubRequest>,
)->Result<Json<serde_json::Value>,(StatusCode,Json<serde_json::Value>)>{
    let lock_index=variable_hub_lock_index(&state,&request.venue,&request.symbol,&request.timeframe);
    let _hub_guard=state.variable_hub_locks[lock_index].lock().await;

    if !timestamps_are_not_future(request.event_time_cutoff_ms,request.available_at_ms){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future market-vector publication timestamps are rejected"}))));
    }
    let mut hub=load_or_create_variable_hub(&state,&request.venue,&request.symbol,&request.timeframe).await
        .map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":error}))))?;
    let vector=hub.publish(request.vector_id,request.venue,request.symbol,request.timeframe,
        request.event_time_cutoff_ms,request.available_at_ms).map_err(variable_hub_error)?;
    let mut document=mongodb::bson::to_document(&vector).map_err(|error|(StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error":error.to_string()}))))?;
    document.insert("stored_at",mongodb::bson::DateTime::now());
    match state.market_vectors.insert_one(document,None).await{
        Ok(_)=>{},
        Err(error) if error.to_string().contains("E11000")=>{
            let existing=state.market_vectors.find_one(doc!{"vector_id":&vector.vector_id},None).await
                .map_err(|lookup_error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"could not verify existing immutable vector","detail":lookup_error.to_string()}))))?;
            let Some(mut existing)=existing else{
                return Err((StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"vector ID collision could not be resolved"}))));
            };
            existing.remove("_id");existing.remove("stored_at");
            let existing=mongodb::bson::from_document::<MarketVector>(existing)
                .map_err(|decode_error|(StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"stored vector is invalid","detail":decode_error.to_string()}))))?;
            if existing!=vector{
                return Err((StatusCode::CONFLICT,Json(serde_json::json!({"error":"market vector IDs are immutable; the existing vector has different content","vector_id":vector.vector_id}))));
            }
        }
        Err(error)=>return Err((StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({"error":"market vector persistence failed","detail":error.to_string()})))),
    }
    persist_variable_hub(&state,&hub).await.map_err(|error|(StatusCode::SERVICE_UNAVAILABLE,
        Json(serde_json::json!({"error":"variable hub persistence failed","detail":error}))))?;
    Ok(Json(serde_json::json!({"market_vector":vector,"variables":hub.progress_all()})))
}

async fn evaluate_market_vector_signal(
    State(state): State<AppState>,
    Json(request): Json<MarketVectorSignalRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let vector_id = request.vector.vector_id.clone();
    let schema_version = request.vector.schema_version;
    if !timestamps_are_not_future(request.vector.event_time_cutoff_ms,request.vector.available_at_ms){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future market vectors cannot produce signals"}))));
    }
    let decision = decide_from_market_vector(
        &request.vector,
        &state.signal_config,
        &state.signal_config_version,
        request.has_open_position,
    ).map_err(|error| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": error.to_string()})),
    ))?;
    Ok((StatusCode::OK, Json(serde_json::json!({
        "algorithm_version": "1",
        "vector_id": vector_id,
        "vector_schema_version": schema_version,
        "decision": decision,
        "live_trading": false
    }))))
}

#[derive(Deserialize)]
struct DiscoverVectorRelationshipsRequest {
    query: MarketVector,
    candidates: Vec<MarketVector>,
    model_version: String,
    temperature: f64,
    recorded_at_ms: i64,
    #[serde(default)]
    evidence_config: Option<VectorEvidenceConfig>,
}

#[derive(Deserialize)]
struct DiscoverVectorHistoryQuery {
    venue:String,
    symbol:String,
    limit:Option<i64>,
    temperature:Option<f64>,
}

/// Build V2 evidence sets directly from immutable vectors already produced by
/// ingestion. Clients do not need to upload or reconstruct market snapshots.
async fn discover_vector_relationships_from_history(
    State(state):State<AppState>,
    Query(query):Query<DiscoverVectorHistoryQuery>,
)->Result<(StatusCode,Json<serde_json::Value>),(StatusCode,Json<serde_json::Value>)>{
    if query.venue.trim().is_empty() || query.venue.len()>64
        || query.symbol.trim().is_empty() || query.symbol.len()>96{
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({
            "error":"venue and symbol are required and must fit the market-vector identity limits"
        }))));
    }
    let limit=query.limit.unwrap_or(21).clamp(2,101);
    let temperature=query.temperature.unwrap_or(1.0);
    if !temperature.is_finite() || temperature<=0.0 || temperature>10.0{
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({
            "error":"temperature must be finite and in (0, 10]"
        }))));
    }
    let filter=doc!{"venue":&query.venue,"symbol":&query.symbol};
    let options=FindOptions::builder()
        .sort(doc!{"event_time_cutoff_ms":-1,"vector_id":1})
        .limit(limit)
        .build();
    let mut cursor=state.market_vectors.find(filter,Some(options)).await.map_err(|error|{
        tracing::error!(error=%error,venue=%query.venue,symbol=%query.symbol,
            "V2 market-vector history lookup failed");
        (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "error":"market-vector history unavailable"
        })))
    })?;
    let mut vectors=Vec::new();
    while let Some(mut document)=cursor.try_next().await.map_err(|error|{
        tracing::error!(error=%error,"V2 market-vector history cursor failed");
        (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
            "error":"market-vector history read failed"
        })))
    })?{
        document.remove("_id");
        document.remove("stored_at");
        let vector=mongodb::bson::from_document::<MarketVector>(document).map_err(|error|{
            tracing::error!(error=%error,"invalid vector in V2 discovery history");
            (StatusCode::SERVICE_UNAVAILABLE,Json(serde_json::json!({
                "error":"market-vector history contains invalid snapshots"
            })))
        })?;
        vectors.push(vector);
    }
    if vectors.len()<2{
        return Ok((StatusCode::ACCEPTED,Json(serde_json::json!({
            "status":"warming_up",
            "source":"persisted_market_vector_history",
            "venue":query.venue,
            "symbol":query.symbol,
            "available_vectors":vectors.len(),
            "required_vectors":2,
            "message":"V2 needs a query snapshot and at least one earlier candidate snapshot"
        }))));
    }
    let query_vector=vectors.remove(0);
    // Use the snapshot availability boundary as the deterministic ledger time.
    // Retrying discovery for the same immutable vector window then resolves to
    // the same event IDs instead of appending duplicate history on every retry.
    let recorded_at_ms=vectors.iter().map(|vector|vector.available_at_ms)
        .chain(std::iter::once(query_vector.available_at_ms)).max().unwrap_or(0);
    let request=DiscoverVectorRelationshipsRequest{
        query:query_vector,
        candidates:vectors,
        model_version:env::var("QOIN_V2_MODEL_VERSION")
            .unwrap_or_else(|_|"set-attention-v1".into()),
        temperature,
        recorded_at_ms,
        evidence_config:None,
    };
    let (status,Json(mut response))=
        discover_vector_relationships(State(state),Json(request)).await?;
    response["source"]=serde_json::json!("persisted_market_vector_history");
    Ok((status,Json(response)))
}

async fn discover_vector_relationships(
    State(state): State<AppState>,
    Json(request): Json<DiscoverVectorRelationshipsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if request.candidates.is_empty() || request.candidates.len() > 500
        || request.recorded_at_ms <= 0 || request.model_version.trim().is_empty()
        || request.model_version.len() > 128
    {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"provide 1-500 candidate market vectors, a model_version, and a positive recorded_at_ms"
        }))));
    }
    let now=chrono::Utc::now().timestamp_millis();
    if request.recorded_at_ms>now || request.query.event_time_cutoff_ms>now || request.query.available_at_ms>now
        || request.candidates.iter().any(|vector|vector.event_time_cutoff_ms>now||vector.available_at_ms>now){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future market vectors or ledger timestamps are rejected"}))));
    }
    let latest_available = request.candidates.iter()
        .map(|vector| vector.available_at_ms)
        .chain(std::iter::once(request.query.available_at_ms))
        .max().unwrap_or(0);
    if request.recorded_at_ms < latest_available {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"recorded_at_ms cannot precede vector availability timestamps"
        }))));
    }

    let config = request.evidence_config.unwrap_or_default();
    let query = EvidenceSet::from_market_vector(&request.query, &config).map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    let candidates = request.candidates.iter()
        .map(|vector| EvidenceSet::from_market_vector(vector, &config))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| (
            StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
        ))?;

    discover_relationships(
        State(state),
        Json(DiscoverRelationshipsRequest {
            query,
            candidates,
            model_version: request.model_version,
            temperature: request.temperature,
            recorded_at_ms: request.recorded_at_ms,
        }),
    ).await
}

#[derive(Deserialize)]
struct RelationshipLedgerQuery { limit: Option<i64> }

#[derive(Deserialize)]
struct DiscoverRelationshipsRequest {
    query: EvidenceSet,
    candidates: Vec<EvidenceSet>,
    model_version: String,
    temperature: f64,
    recorded_at_ms: i64,
}

async fn discover_relationships(
    State(state): State<AppState>,
    Json(request): Json<DiscoverRelationshipsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if request.candidates.is_empty() || request.candidates.len() > 500
        || request.recorded_at_ms <= 0 || request.model_version.trim().is_empty()
        || request.model_version.len() > 128
    {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"provide 1-500 candidate sets, a model_version, and a positive recorded_at_ms"
        }))));
    }
    let now=chrono::Utc::now().timestamp_millis();
    if request.recorded_at_ms>now || request.query.observed_at_ms>now
        || request.candidates.iter().any(|candidate|candidate.observed_at_ms>now){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future evidence or ledger timestamps are rejected"}))));
    }
    request.query.validate().map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    for candidate in &request.candidates {
        candidate.validate().map_err(|error| (
            StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
        ))?;
    }
    let total_atoms = request.query.atoms.len()
        + request.candidates.iter().map(|candidate| candidate.atoms.len()).sum::<usize>();
    if total_atoms > 10_000 {
        return Err((StatusCode::PAYLOAD_TOO_LARGE, Json(serde_json::json!({
            "error":"total evidence atoms per discovery request cannot exceed 10000"
        }))));
    }
    let latest_observation = request.candidates.iter()
        .map(|candidate| candidate.observed_at_ms)
        .chain(std::iter::once(request.query.observed_at_ms))
        .max().unwrap_or(0);
    if request.recorded_at_ms < latest_observation {
        return Err((StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error":"recorded_at_ms cannot precede the evidence observation timestamps"
        }))));
    }

    let mut evidence_sets = Vec::with_capacity(request.candidates.len() + 1);
    evidence_sets.push(request.query.clone());
    evidence_sets.extend(request.candidates.iter().cloned());
    let vocabulary = Vocabulary::from_sets(&evidence_sets).map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    let query_embedding = vocabulary.embed(&request.query).map_err(|error| (
        StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":error.to_string()}))
    ))?;
    let key_embeddings = request.candidates.iter()
        .map(|candidate| vocabulary.embed(candidate))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
    let weights = set_attention_weights(&query_embedding, &key_embeddings, request.temperature)
        .map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;

    let mut persisted = 0usize;
    let mut already_present = 0usize;
    let mut relationships = Vec::with_capacity(request.candidates.len());
    for (candidate, attention_weight) in request.candidates.iter().zip(weights) {
        let relationship = RelationshipRecord::discover(
            &request.query, candidate, request.model_version.clone(), attention_weight,
            request.temperature,
        ).map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
        relationships.push(relationship.clone());
        let identity = serde_json::to_vec(&(
            &relationship,
            request.recorded_at_ms,
        )).map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":format!("could not encode relationship identity: {error}")}))))?;
        let event_id = format!("v2rel:{}", hex::encode(Sha256::digest(identity)));
        let event = RelationshipLedgerEvent {
            event_id,
            event_type: RelationshipEventType::Discovered,
            recorded_at_ms: request.recorded_at_ms,
            relationship,
            outcome: None,
        };
        event.validate().map_err(|error| (StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":error.to_string()}))))?;
        let document = mongodb::bson::to_document(&event).map_err(|error| (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":format!("could not serialize ledger event: {error}")})),
        ))?;
        match state.relationship_ledger.insert_one(document, None).await {
            Ok(_) => persisted += 1,
            Err(error) if error.to_string().contains("E11000") => already_present += 1,
            Err(error) => {
                tracing::error!(error=%error, persisted, already_present,
                    "V2 relationship discovery ledger append failed");
                return Err((StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({
                    "error":"relationship ledger append failed",
                    "persisted_before_failure":persisted,
                    "already_present":already_present
                }))));
            }
        }
    }
    let status = if persisted > 0 { StatusCode::CREATED } else { StatusCode::OK };
    Ok((status, Json(serde_json::json!({
        "model_version":request.model_version,
        "candidate_count":request.candidates.len(),
        "persisted":persisted,
        "already_present":already_present,
        "relationships":relationships,
        "ledger":"append-only",
        "decision_authority":"version-2-set-theoretic-attention (research only)"
    }))))
}

async fn append_relationship_event(
    State(state): State<AppState>,
    Json(event): Json<RelationshipLedgerEvent>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    if event.recorded_at_ms>chrono::Utc::now().timestamp_millis(){
        return Err((StatusCode::BAD_REQUEST,Json(serde_json::json!({"error":"future ledger event timestamps are rejected"}))));
    }
    event.validate().map_err(|error| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": error.to_string()})),
    ))?;
    let event_id = event.event_id.clone();
    let document = mongodb::bson::to_document(&event).map_err(|error| (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({"error": format!("could not serialize ledger event: {error}")})),
    ))?;
    match state.relationship_ledger.insert_one(document, None).await {
        Ok(_) => Ok((StatusCode::CREATED, Json(serde_json::json!({
            "event_id": event_id,
            "persisted": true,
            "append_only": true
        })))),
        Err(error) if error.to_string().contains("E11000") => Err((
            StatusCode::CONFLICT,
            Json(serde_json::json!({"error":"relationship ledger event_id already exists"})),
        )),
        Err(error) => {
            tracing::error!(error=%error, event_id=%event_id, "relationship ledger append failed");
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"relationship ledger unavailable"})),
            ))
        }
    }
}

async fn list_relationship_events(
    State(state): State<AppState>,
    Query(query): Query<RelationshipLedgerQuery>,
) -> Result<Json<Vec<RelationshipLedgerEvent>>, (StatusCode, Json<serde_json::Value>)> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let options = FindOptions::builder()
        .sort(doc!{"recorded_at_ms":-1,"event_id":1})
        .limit(limit)
        .build();
    let mut cursor = state.relationship_ledger.find(None, Some(options)).await.map_err(|error| {
        tracing::error!(error=%error, "relationship ledger query failed");
        (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"relationship ledger unavailable"})))
    })?;
    let documents = cursor.try_collect::<Vec<Document>>().await.map_err(|error| {
        tracing::error!(error=%error, "relationship ledger read failed");
        (StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error":"relationship ledger read failed"})))
    })?;
    let events = documents.into_iter()
        .map(mongodb::bson::from_document::<RelationshipLedgerEvent>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            tracing::error!(error=%error, "relationship ledger contains an invalid persisted event");
            (StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"error":"relationship ledger contains invalid data"})))
        })?;
    Ok(Json(events))
}

#[tokio::main]
async fn main(){
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init();
    let mut signal_config = EngineConfig::default();
    if let Ok(value) = env::var("QOIN_SIGNAL_THRESHOLD") { signal_config.signal_threshold = value.parse().expect("QOIN_SIGNAL_THRESHOLD must be numeric"); }
    if let Ok(value) = env::var("QOIN_INPUT_CLIP") { signal_config.input_clip = value.parse().expect("QOIN_INPUT_CLIP must be numeric"); }
    if let Ok(value) = env::var("QOIN_TOP_K") { signal_config.top_k = value.parse().expect("QOIN_TOP_K must be an integer"); }
    if let Ok(value) = env::var("QOIN_MIN_ALIGNMENT") { signal_config.min_alignment = value.parse().expect("QOIN_MIN_ALIGNMENT must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_ANGLE_RADIANS") { signal_config.max_angle_radians = value.parse().expect("QOIN_MAX_ANGLE_RADIANS must be numeric"); }
    let signal_config_version = env::var("QOIN_CONFIG_VERSION").unwrap_or_else(|_| "baseline-v2".into());
    decide_signal(ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
        open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0 },
        &signal_config, &signal_config_version, false).expect("Qoin signal engine configuration must be valid");

    let uri=env::var("QOIN_MONGODB_URI").unwrap_or_else(|_|"mongodb://127.0.0.1:27017".into());
    let client=Client::with_uri_str(&uri).await.expect("connect to MongoDB");
    let db=client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_|"qoin".into()));
    db.run_command(doc!{"ping":1},None).await.expect("MongoDB ping");
    let events:Collection<Document>=db.collection("market_events");
    let states:Collection<Document>=db.collection("market_states");
    let market_vectors:Collection<Document>=db.collection("market_vectors");
    let variable_hubs:Collection<Document>=db.collection("market_variable_hubs");
    let decisions:Collection<Document>=db.collection("trade_decisions");
    let strategies:Collection<Document>=db.collection("edge_strategy");
    let relationship_ledger:Collection<Document>=db.collection("v2_relationship_ledger");
    let orders:Collection<Document>=db.collection("live_orders");
    let trading_controls:Collection<Document>=db.collection("trading_controls");
    trading_controls.update_one(doc!{"_id":"execution"},
        doc!{"$setOnInsert":{"enabled":env::var("QOIN_TRADING_ENABLED").map(|v|v=="true").unwrap_or(false),
            "updated_at":mongodb::bson::DateTime::now()}},
        UpdateOptions::builder().upsert(true).build()).await.expect("initialize trading kill switch");
    orders.create_index(mongodb::IndexModel::builder().keys(doc!{"client_order_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("durable live order idempotency index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"ts_ms":1,"event_id":1}).build(),None).await.expect("historical replay time index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"ts_ms":1,"_id":1}).build(),None)
        .await.expect("globally ordered event replay index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"_qoin_pipeline_status":1}).build(),None)
        .await.expect("pending event pipeline health index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"stored_at":-1}).build(),None).await.expect("latest state recovery index");
    market_vectors.create_index(mongodb::IndexModel::builder().keys(doc!{"vector_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("market vector idempotency index");
    market_vectors.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"event_time_cutoff_ms":-1}).build(),None)
        .await.expect("latest market vector lookup index");
    market_vectors.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_time_cutoff_ms":-1}).build(),None)
        .await.expect("venue-scoped V2 vector history index");
    variable_hubs.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"identity.venue":1,"identity.symbol":1,"identity.timeframe":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None)
        .await.expect("variable hub identity index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"decision_key":1})
        .options(IndexOptions::builder().unique(true).sparse(true).build()).build(),None)
        .await.expect("decision recovery idempotency index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    relationship_ledger.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("relationship ledger event idempotency index");
    relationship_ledger.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"recorded_at_ms":-1,"event_id":1}).build(),None).await
        .expect("relationship ledger chronological index");
    let baseline=EdgeStrategy::baseline();
    let active=match strategies.find_one(doc!{"strategy_key":STRATEGY_KEY},None).await.expect("load edge strategy map"){
        Some(doc)=>match mongodb::bson::from_document::<EdgeStrategy>(doc){Ok(v)=>{v.validate().expect("stored edge strategy must validate");v},
            Err(e)=>panic!("stored edge strategy map is invalid: {e}")},
        None=>{
            let d=mongodb::bson::to_document(&baseline).expect("serialize baseline strategy map");
            strategies.insert_one(d,None).await.expect("seed baseline edge strategy map");
            baseline
        }
    };
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded active edge strategy map from MongoDB");
    // Rebuild rolling state from the immutable event log on every process start.
    // Snapshot-only recovery can preserve a stale snapshot when a crash occurs
    // after event insertion but before snapshot persistence. Replaying the raw
    // log closes that specific recovery gap and uses deterministic event-time
    // ordering across providers. Malformed legacy rows are skipped and surfaced.
    let replay_options = FindOptions::builder()
        // Preserve live per-symbol arrival order for equal timestamps while
        // retaining global event-time ordering across symbols.
        .sort(doc!{"ts_ms":1,"_id":1})
        .build();
    let mut event_cursor = events.find(None, Some(replay_options)).await
        .expect("open immutable market event log for recovery");
    let mut restored_memory: HashMap<String, SymbolState> = HashMap::new();
    let mut restored_vectors: HashMap<String, MarketVector> = HashMap::new();
    let mut restored_latest_events: HashMap<String, MarketEvent> = HashMap::new();
    let mut replayed_events: u64 = 0;
    let mut skipped_events: u64 = 0;
    while let Some(document) = event_cursor.try_next().await
        .expect("read immutable market event log during recovery") {
        let persisted_available_at_ms=document.get_i64("_qoin_available_at_ms").ok();
        if !persisted_event_is_applied(&document){
            tracing::debug!("preserving persisted late event as unapplied during recovery");
            continue;
        }
        match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event) if validate_event(&event).is_ok() => {
                let state_key=format!("{}|{}",event.venue,event.symbol);
                let venue_prefix=format!("{}|",event.venue);
                let peer_states=restored_memory.iter()
                    .filter(|(key,_)|key.starts_with(&venue_prefix)&&*key!=&state_key)
                    .map(|(_,peer)|peer.clone()).collect::<Vec<_>>();
                let state = restored_memory.entry(state_key.clone()).or_default();
                apply(state, &event);
                restored_latest_events.insert(state_key.clone(),event.clone());
                let available_at_ms=persisted_available_at_ms.unwrap_or(event.ts_ms).max(event.ts_ms);
                if let Some(vector)=build_market_vector(state,&event,available_at_ms,&peer_states){
                    if should_evaluate_v1(state.last_v1_evaluated_at_ms,event.ts_ms){
                        state.last_v1_decision=Some(evaluate_ingested_v1_vector(
                            &vector,&signal_config,&signal_config_version
                        ));
                        state.last_v1_evaluated_at_ms=Some(event.ts_ms);
                    }
                    let mut vector_document=mongodb::bson::to_document(&vector)
                        .expect("serialize replayed causal market vector");
                    vector_document.insert("stored_at",mongodb::bson::DateTime::now());
                    match market_vectors.insert_one(vector_document,None).await{
                        Ok(_)=>{},
                        Err(error) if error.to_string().contains("E11000")=>{},
                        Err(error)=>{
                            skipped_events+=1;
                            tracing::warn!(error=%error,vector_id=%vector.vector_id,
                                "could not persist replayed market vector");
                        }
                    }
                    restored_vectors.insert(state_key,vector);
                }
                replayed_events += 1;
            }
            Ok(event) => {
                skipped_events += 1;
                tracing::warn!(symbol=%event.symbol,event_id=%event.event_id,
                    "skipping invalid persisted market event during recovery");
            }
            Err(error) => {
                skipped_events += 1;
                tracing::warn!(error=%error,
                    "skipping malformed persisted market event during recovery");
            }
        }
    }
    // Rebuild the latest decision per symbol as well as its vector. This
    // repairs a crash after raw-event insertion but before decision persistence.
    for (state_key,snapshot) in &restored_memory{
        let Some(event)=restored_latest_events.get(state_key) else{continue;};
        let mut decision=match active.decide(
            &snapshot.symbol,snapshot.order_flow_imbalance,snapshot.funding_rate,
            snapshot.spread_bps,snapshot.ewma_abs_return,snapshot.event_intensity,
            snapshot.liquidation_events,snapshot.last_liquidation_side.as_deref(),
        ){
            Ok(value)=>value,
            Err(error)=>{
                skipped_events+=1;
                tracing::warn!(error=%error,symbol=%snapshot.symbol,
                    "could not reconstruct latest decision during event-log recovery");
                continue;
            }
        };
        decision.input_event=Some(DecisionInputEvent{
            event_id:event.event_id.clone(),venue:event.venue.clone(),event_type:event.event_type.clone(),
            event_ts_ms:event.ts_ms,state_event_count:snapshot.event_count,
            state_last_event_ts_ms:snapshot.last_event_ts_ms,
        });
        let v1=cached_v1_decision(snapshot,event.ts_ms);
        let mut document=mongodb::bson::to_document(&decision).unwrap_or_default();
        let key=market_event_decision_key(event);
        document.insert("decision_key",&key);
        document.insert("v1_shared_market_vector",mongodb::bson::to_bson(&v1).unwrap_or(mongodb::bson::Bson::Null));
        if let Some(vector)=restored_vectors.get(state_key){document.insert("market_vector_id",&vector.vector_id);}
        document.insert("recovered_from_event_log",true);
        document.insert("stored_at",mongodb::bson::DateTime::now());
        if let Err(error)=decisions.update_one(
            doc!{"decision_key":&key},doc!{"$set":document},
            UpdateOptions::builder().upsert(true).build(),
        ).await{
            skipped_events+=1;
            tracing::warn!(error=%error,symbol=%snapshot.symbol,
                "could not persist reconstructed latest decision");
        }
    }
    if skipped_events==0{
        if let Err(error)=events.update_many(
            doc!{"_qoin_pipeline_status":"pending","_qoin_applied":true},
            doc!{"$set":{"_qoin_pipeline_status":"complete","_qoin_recovered_at":mongodb::bson::DateTime::now()}},
            None,
        ).await{
            skipped_events+=1;
            tracing::warn!(error=%error,"could not complete recovered market-event pipeline markers");
        }
    }
    tracing::info!(restored_symbols=restored_memory.len(),replayed_events,skipped_events,
        "rebuilt market states, vectors and latest decisions from immutable MongoDB event log");
    let (stream, _) = broadcast::channel(4096);
    let state=AppState{
        stream, database: db.clone(), events,states,market_vectors,variable_hubs,
        variable_hub_locks:Arc::new((0..64).map(|_|tokio::sync::Mutex::new(())).collect()),
        decisions,strategies,relationship_ledger,orders,trading_controls,
        memory:Arc::new(RwLock::new(restored_memory)),
        symbol_ingest_locks:Arc::new((0..64).map(|_| tokio::sync::Mutex::new(())).collect()),
        active_strategy:Arc::new(RwLock::new(active)),
        signal_config,
        signal_config_version,
        recovery_replayed_events: replayed_events,
        recovery_skipped_events: skipped_events,
    };
    let app=Router::new().route("/health",get(health)).route("/v1/signal",post(evaluate_signal))
        .route("/internal/v1/market-vector/signal",post(evaluate_market_vector_signal))
        .route("/internal/orders",post(submit_live_order))
        .route("/internal/orders/:client_order_id/reconcile",get(reconcile_live_order))
        .route("/internal/trading/kill-switch",post(set_trading_enabled))
        .route("/internal/stream",get(stream_ws)).route("/internal/events",get(get_events).post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/market-vector",get(get_latest_market_vector))
        .route("/internal/vector/history",get(get_vector_history))
        .route("/internal/market-vector/history",get(get_market_vector_history))
        .route("/internal/variables/observe",post(observe_variable))
        .route("/internal/variables/normalize",post(normalize_variable))
        .route("/internal/variables/derive",post(derive_variable))
        .route("/internal/variables/not-applicable",post(mark_variable_not_applicable))
        .route("/internal/variables/progress",get(get_variable_progress))
        .route("/internal/variables/publish",post(publish_variable_hub))
        .route("/internal/backtest",get(run_backtest)).route("/internal/instruments",get(get_instruments))
        .route("/internal/news",get(get_news)).route("/internal/calendar",get(get_calendar))
        .route("/internal/v2/relationships",get(list_relationship_events).post(append_relationship_event))
        .route("/internal/v2/relationships/discover",post(discover_relationships))
        .route("/internal/v2/relationships/discover-vectors",post(discover_vector_relationships))
        .route("/internal/v2/relationships/discover-history",post(discover_vector_relationships_from_history))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .route("/internal/algorithms",get(algorithm_catalogue::get_algorithms))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    // Bind address may be 0.0.0.0 inside a container; provider callbacks must
    // connect through a concrete local interface instead of the wildcard address.
    let internal_addr=env::var("QOIN_AXUM_INTERNAL_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let binance_enabled=env::var("QOIN_BINANCE_STREAM_ENABLED").map(|v|v!="false").unwrap_or(true);
    if binance_enabled {
        let symbols=env::var("QOIN_BINANCE_STREAM_SYMBOLS").unwrap_or_else(|_|"BTCUSDT,ETHUSDT,SOLUSDT,BNBUSDT,XRPUSDT,DOGEUSDT".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(|v|v.to_ascii_uppercase()).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        let open_interest_addr=internal_addr.clone();
        let open_interest_symbols=symbols.clone();
        tokio::spawn(async move{spawn_binance_stream(adapter_addr,symbols).await;});
        tokio::spawn(async move{spawn_binance_open_interest(open_interest_addr,open_interest_symbols).await;});
    } else { tracing::info!("Binance public market stream disabled by configuration"); }
    let kraken_enabled=env::var("QOIN_KRAKEN_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
    if kraken_enabled {
        let symbols=env::var("QOIN_KRAKEN_STREAM_SYMBOLS").unwrap_or_else(|_|"BTC/USD,ETH/USD,SOL/USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_kraken_stream(adapter_addr,symbols).await; });
    } else {
        tracing::info!("Kraken Spot public market stream disabled by configuration");
    }
    let coinbase_enabled=env::var("QOIN_COINBASE_STREAM_ENABLED").map(|v|v=="true").unwrap_or(false);
    if coinbase_enabled {
        let products=env::var("QOIN_COINBASE_STREAM_PRODUCTS").unwrap_or_else(|_|"BTC-USD,ETH-USD,SOL-USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_coinbase_stream(adapter_addr,products).await; });
    } else {
        tracing::info!("Coinbase public market stream disabled by configuration");
    }
    if let Ok(token) = env::var("FINNHUB_API_KEY") {
      if !token.trim().is_empty() {
        let symbols=env::var("QOIN_FINNHUB_STREAM_SYMBOLS").unwrap_or_else(|_|"AAPL,MSFT,NVDA,AMZN,GOOGL,META,TSLA,OANDA:EUR_USD,OANDA:GBP_USD,OANDA:USD_JPY,OANDA:AUD_USD,OANDA:SPX500_USD,OANDA:NAS100_USD,OANDA:US30_USD,OANDA:DE30_EUR,OANDA:XAU_USD,OANDA:XAG_USD,OANDA:WTICO_USD,OANDA:BCO_USD,OANDA:NATGAS_USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_finnhub_stream(adapter_addr,token,symbols).await; });
      } else { tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is empty"); }
    } else {
        tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is not configured");
    }
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}

#[cfg(test)]
mod market_stream_tests {
    use super::*;

    #[test]
    fn kraken_trade_messages_normalize_into_canonical_events() {
        let payload=serde_json::json!({
            "channel":"trade","type":"update","data":[{
                "symbol":"BTC/USD","side":"buy","price":65000.25,"qty":0.01,
                "trade_id":123456,"timestamp":"2026-11-09T12:00:00.123Z"
            }]
        });
        let events=parse_kraken_trade_message(&payload);
        assert_eq!(events.len(),1);
        assert_eq!(events[0].venue,"kraken_spot");
        assert_eq!(events[0].symbol,"BTCUSD_KRAKEN");
        assert_eq!(events[0].event_id,"kraken_trade:123456");
        assert_eq!(events[0].side.as_deref(),Some("buy"));
        assert_eq!(events[0].ts_ms,1_794_225_600_123);
    }

    #[test]
    fn kraken_parser_rejects_snapshots_and_malformed_trades() {
        assert!(parse_kraken_trade_message(&serde_json::json!({"channel":"trade","type":"snapshot","data":[]})).is_empty());
        let payload=serde_json::json!({"channel":"trade","type":"update","data":[{
            "symbol":"BTC/USD","side":"unknown","price":"1","qty":"1",
            "trade_id":1,"timestamp":"not-a-time"
        }]});
        assert!(parse_kraken_trade_message(&payload).is_empty());
    }

    #[test]
    fn coinbase_market_trade_messages_normalize_into_canonical_events() {
        let payload=serde_json::json!({
            "channel":"market_trades",
            "events":[{"type":"update","trades":[{
                "trade_id":"12345","product_id":"BTC-USD","price":"65000.25",
                "size":"0.012","side":"BUY","time":"2026-11-09T12:00:00.123Z"
            }]}]
        });
        let events=parse_coinbase_trade_message(&payload);
        assert_eq!(events.len(),1);
        assert_eq!(events[0].venue,"coinbase_advanced_trade");
        assert_eq!(events[0].symbol,"BTCUSD_COINBASE");
        assert_eq!(events[0].event_id,"coinbase_trade:12345");
        assert_eq!(events[0].side.as_deref(),Some("buy"));
        assert_eq!(events[0].ts_ms,1_794_225_600_123);
    }

    #[test]
    fn coinbase_parser_rejects_wrong_channels_and_malformed_trades() {
        assert!(parse_coinbase_trade_message(&serde_json::json!({"channel":"ticker","events":[]})).is_empty());
        let payload=serde_json::json!({
            "channel":"market_trades",
            "events":[{"trades":[{"trade_id":"bad","product_id":"BTC-USD","side":"BUY",
                "time":"not-a-time","price":"NaN","size":"1"}]}]
        });
        assert!(parse_coinbase_trade_message(&payload).is_empty());
    }

    #[test]
    fn websocket_symbol_subscriptions_are_trimmed_uppercase_and_deduplicated() {
        let symbols = subscription_set(None, Some(" btcusdt, ETHUSDT,btcusdt, ,solusdt ".into()));
        assert_eq!(symbols.len(), 3);
        assert!(symbols.contains("BTCUSDT"));
        assert!(symbols.contains("ETHUSDT"));
        assert!(symbols.contains("SOLUSDT"));
    }

    #[test]
    fn stream_numbers_accept_provider_string_and_numeric_values() {
        assert_eq!(stream_number(Some(&serde_json::json!("123.45"))), Some(123.45));
        assert_eq!(stream_number(Some(&serde_json::json!(0.25))), Some(0.25));
        assert_eq!(stream_number(Some(&serde_json::json!("not-a-number"))), None);
        assert_eq!(stream_number(None), None);
    }

    #[test]
    fn market_event_validation_rejects_unknown_types_and_invalid_sides() {
        let base = MarketEvent {
            event_id: "test-1".into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms: 1_700_000_000_000, price: Some(100.0),
            size: Some(1.0), side: Some("buy".into()), bid: None, ask: None,
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None,mark_price:None,index_price:None,
            liquidation_side: None,
        };
        assert!(validate_event(&base).is_ok());
        let mut unknown = base.clone();
        unknown.event_type = "order".into();
        assert!(validate_event(&unknown).is_err());
        let mut invalid_side = base.clone();
        invalid_side.side = Some("BUY".into());
        assert!(validate_event(&invalid_side).is_err());
        let mut invalid_liquidation = base;
        invalid_liquidation.liquidation_side = Some("up".into());
        assert!(validate_event(&invalid_liquidation).is_err());
    }

    #[test]
    fn market_event_validation_rejects_non_finite_and_crossed_book_data() {
        let base = MarketEvent {
            event_id: "test-1".into(), venue: "test".into(), symbol: "BTCUSDT".into(),
            event_type: "trade".into(), ts_ms: 1_700_000_000_000, price: Some(100.0),
            size: Some(1.0), side: Some("buy".into()), bid: None, ask: None,
            bid_size: None, ask_size: None, funding_rate: None, open_interest: None,mark_price:None,index_price:None,
            liquidation_side: None,
        };
        assert!(validate_event(&base).is_ok());
        let mut non_finite = base.clone();
        non_finite.price = Some(f64::NAN);
        assert!(validate_event(&non_finite).is_err());
        let mut crossed = base;
        crossed.bid = Some(101.0);
        crossed.ask = Some(100.0);
        assert!(validate_event(&crossed).is_err());
    }
    #[test]
    fn causal_normalization_requires_past_samples_and_rejects_constant_history(){
        let mut history=VecDeque::new();
        for value in 1..=7{assert!(causal_robust_normalize(value as f64,&history).is_none());push_bounded(&mut history,value as f64);}
        push_bounded(&mut history,8.0);
        assert!(causal_robust_normalize(100.0,&history).unwrap()>0.99);
        assert!(causal_robust_normalize(2.0,&VecDeque::from(vec![2.0;12])).is_none());
    }
    #[test]
    fn event_pipeline_publishes_only_causally_normalized_features(){
        let mut state=SymbolState::default();let mut latest=None;
        for index in 0..24{
            let price=100.0+index as f64*0.15+(index%3) as f64*0.08;
            let event=MarketEvent{
                event_id:format!("pipeline-{index}"),venue:"test-venue".into(),symbol:"BTCUSDT".into(),
                event_type:"trade".into(),ts_ms:1_800_000_000_000+index*1_000,price:Some(price),size:Some(1.0),
                side:Some(if index%2==0{"buy"}else{"sell"}.into()),bid:None,ask:None,bid_size:None,ask_size:None,
                funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut state,&event);latest=build_market_vector(&mut state,&event,event.ts_ms,&[]);
        }
        let vector=latest.expect("past samples should warm the vector");
        assert!(vector.validate().is_ok());assert_eq!(vector.symbol,"BTCUSDT");assert_eq!(vector.venue,"test-venue");
        assert!(vector.event_time_cutoff_ms<=vector.available_at_ms);
        assert_eq!(vector.coordinates[&MarketFeature::RegimeCorrelationStress].status,qoin_market_space::FeatureAvailability::Missing);
        assert_eq!(vector.coordinates[&MarketFeature::Basis].status,qoin_market_space::FeatureAvailability::NotApplicable);
        assert_eq!(vector.coordinates[&MarketFeature::PortfolioExposure].status,qoin_market_space::FeatureAvailability::NotApplicable);
        let stale_event=MarketEvent{
            event_id:"stale-feed".into(),venue:"test-venue".into(),symbol:"BTCUSDT".into(),
            event_type:"funding".into(),ts_ms:1_800_000_000_000+24*1_000+120_000,price:None,size:None,
            side:None,bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:Some(0.0001),
            open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
        };
        apply(&mut state,&stale_event);
        let after_stale=build_market_vector(&mut state,&stale_event,stale_event.ts_ms,&[]).unwrap();
        assert_eq!(after_stale.coordinates[&MarketFeature::LogReturn].status,qoin_market_space::FeatureAvailability::Missing);
    }






    #[test]
    fn cross_asset_alignment_excludes_partial_and_future_buckets(){
        let mut current=SymbolState::default();
        current.venue="binance_usdm".into();current.symbol="BTCUSDT".into();
        let mut peer=SymbolState::default();
        peer.venue="binance_usdm".into();peer.symbol="ETHUSDT".into();
        for index in 1..=15_i64{
            let timestamp=index*60_000+1_000;
            current.recent_return_observations.push_back((timestamp,index as f64*0.0001));
            peer.recent_return_observations.push_back((timestamp,index as f64*0.0002));
        }
        current.last_price_ts_ms=Some(15*60_000+1_000);
        peer.recent_return_observations.push_back((16*60_000+1_000,0.01));
        peer.last_price_ts_ms=Some(16*60_000+1_000);
        let partial=minute_return_buckets(&current,15*60_000+2_000);
        assert!(!partial.contains_key(&15));
        assert!(partial.contains_key(&14));
        let features=cross_asset_raw_features(&current,&[peer],15*60_000+2_000);
        assert!(features.is_empty(),"future peer observations must not leak into the current vector");
    }

    #[test]
    fn derivative_fixed_scales_keep_constant_valid_observations_observed(){
        assert!((fixed_scale_feature(MarketFeature::FundingPressure,0.0001).unwrap()-0.761594).abs()<1.0e-5);
        assert!(fixed_scale_feature(MarketFeature::OpenInterestChange,0.0).unwrap()==0.0);
        assert!(fixed_scale_feature(MarketFeature::Basis,0.001).unwrap()>0.7);
    }

    #[test]
    fn cross_asset_features_use_aligned_same_venue_returns(){
        let mut current=SymbolState::default();
        current.venue="binance_usdm".into();
        current.symbol="BTCUSDT".into();
        let mut peer=SymbolState::default();
        peer.venue="binance_usdm".into();
        peer.symbol="ETHUSDT".into();
        let mut foreign=SymbolState::default();
        foreign.venue="kraken_spot".into();
        foreign.symbol="ETHUSD_KRAKEN".into();
        for index in 0..24_i64{
            let base=(index as f64-11.5)*0.0001+(index%3) as f64*0.00003;
            let timestamp=(index+1)*60_000;
            current.recent_return_observations.push_back((timestamp,base));
            peer.recent_return_observations.push_back((timestamp,base*1.8+0.00001));
            foreign.recent_return_observations.push_back((timestamp,-base));
        }
        current.last_price_ts_ms=Some(24*60_000);
        peer.last_price_ts_ms=Some(24*60_000);
        foreign.last_price_ts_ms=Some(24*60_000);
        let features=cross_asset_raw_features(&current,&[peer,foreign],24*60_000);
        assert!(features[&MarketFeature::Correlation]>0.99);
        assert!(features[&MarketFeature::RegimeCorrelationStress]>0.99);
        assert!(features.contains_key(&MarketFeature::CorrelationBreakdown));
    }

    #[test]
    fn non_price_events_do_not_duplicate_price_samples_in_calibration(){
        let mut state=SymbolState::default();
        for index in 0..12{
            let event=MarketEvent{
                event_id:format!("price-{index}"),venue:"test-venue".into(),symbol:"BTCUSDT".into(),
                event_type:"trade".into(),ts_ms:1_800_000_000_000+index*1_000,
                price:Some(100.0+index as f64),size:Some(1.0),side:Some("buy".into()),
                bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,
                open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut state,&event);
            let _=build_market_vector(&mut state,&event,event.ts_ms,&[]);
        }
        let before=state.feature_history[&MarketFeature::LogReturn].len();
        let book=MarketEvent{
            event_id:"book-only".into(),venue:"test-venue".into(),symbol:"BTCUSDT".into(),
            event_type:"book".into(),ts_ms:1_800_000_000_000+12_000,
            price:None,size:None,side:None,bid:Some(99.9),ask:Some(100.1),
            bid_size:Some(10.0),ask_size:Some(11.0),funding_rate:None,
            open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
        };
        apply(&mut state,&book);
        let _=build_market_vector(&mut state,&book,book.ts_ms,&[]);
        assert_eq!(state.feature_history[&MarketFeature::LogReturn].len(),before);
    }


    #[test]
    fn binance_event_sequence_builds_a_v1_ready_vector_with_real_required_inputs(){
        let mut current=SymbolState::default();
        let mut peer=SymbolState::default();
        current.venue="binance_usdm".into();current.symbol="BTCUSDT".into();
        peer.venue="binance_usdm".into();peer.symbol="ETHUSDT".into();
        let mut latest=None;
        let base=1_800_000_000_000_i64;
        for index in 0..36_i64{
            let minute=base+index*60_000;
            let price=100.0+index as f64*0.18+(index%5) as f64*0.07;
            let peer_price=200.0+index as f64*0.36+(index%5) as f64*0.14;
            let trade=MarketEvent{
                event_id:format!("btc-trade-{index}"),venue:"binance_usdm".into(),symbol:"BTCUSDT".into(),
                event_type:"trade".into(),ts_ms:minute+1_000,price:Some(price),size:Some(1.0+(index%4) as f64),
                side:Some(if index%2==0{"buy"}else{"sell"}.into()),bid:None,ask:None,bid_size:None,ask_size:None,
                funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut current,&trade);
            let peer_trade=MarketEvent{
                event_id:format!("eth-trade-{index}"),venue:"binance_usdm".into(),symbol:"ETHUSDT".into(),
                event_type:"trade".into(),ts_ms:minute+1_000,price:Some(peer_price),size:Some(2.0),
                side:Some(if index%2==0{"buy"}else{"sell"}.into()),bid:None,ask:None,bid_size:None,ask_size:None,
                funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut peer,&peer_trade);
            let spread_fraction=0.0001+index as f64*0.000001;
            let book=MarketEvent{
                event_id:format!("btc-book-{index}"),venue:"binance_usdm".into(),symbol:"BTCUSDT".into(),
                event_type:"book".into(),ts_ms:minute+2_000,price:None,size:None,side:None,
                bid:Some(price*(1.0-spread_fraction)),ask:Some(price*(1.0+spread_fraction)),
                bid_size:Some(20.0+index as f64),ask_size:Some(18.0+(index%7) as f64),
                funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut current,&book);
            let derivative=MarketEvent{
                event_id:format!("btc-mark-{index}"),venue:"binance_usdm".into(),symbol:"BTCUSDT".into(),
                event_type:"funding".into(),ts_ms:minute+3_000,price:None,size:None,side:None,
                bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:Some(0.0001+index as f64*0.000001),
                open_interest:None,mark_price:Some(price*1.0003),index_price:Some(price*1.0001),liquidation_side:None,
            };
            apply(&mut current,&derivative);
            let oi=MarketEvent{
                event_id:format!("btc-oi-{index}"),venue:"binance_usdm".into(),symbol:"BTCUSDT".into(),
                event_type:"open_interest".into(),ts_ms:minute+4_000,price:None,size:None,side:None,
                bid:None,ask:None,bid_size:None,ask_size:None,funding_rate:None,
                open_interest:Some(10_000.0+index as f64*17.0+(index%3) as f64),
                mark_price:None,index_price:None,liquidation_side:None,
            };
            apply(&mut current,&oi);
            latest=build_market_vector(&mut current,&oi,minute+5_000,&[peer.clone()]);
        }
        let vector=latest.expect("supported Binance event families should produce a vector");
        for feature in [
            MarketFeature::OrderFlowImbalance,MarketFeature::OrderBookDepth,
            MarketFeature::RealizedVolatility,MarketFeature::FundingPressure,
            MarketFeature::OpenInterestChange,MarketFeature::LiquidationPressure,
            MarketFeature::Basis,MarketFeature::RegimeVolatilityLevel,
            MarketFeature::RegimeLiquidityStress,MarketFeature::RegimeCorrelationStress,
            MarketFeature::RegimeTransitionIntensity,MarketFeature::RegimeUncertainty,
        ]{
            assert_eq!(vector.coordinates[&feature].status,FeatureAvailability::Observed,
                "required feature {feature:?} should be produced from the test event stream");
        }
        let result=evaluate_ingested_v1_vector(&vector,&EngineConfig::default(),"v1-ingestion-test");
        assert_eq!(result["status"],"evaluated");
        assert_eq!(result["decision"]["attribution"]["contributions"].as_array().unwrap().len(),12);
    }

    #[test]
    fn ingested_v1_vector_abstains_with_explicit_required_feature_gaps(){
        let mut vector=MarketVector::new(
            "ingest-v1-not-ready","test-venue","BTCUSDT","event-window",
            1_800_000_000_000,1_800_000_000_001,
        ).unwrap();
        vector.set_value(MarketFeature::OrderFlowImbalance,Some(0.5),0.9).unwrap();
        let result=evaluate_ingested_v1_vector(&vector,&EngineConfig::default(),"v1-test");
        assert_eq!(result["status"],"not_ready");
        assert_eq!(result["missing_required_features"].as_array().unwrap().len(),11);
        assert_eq!(result["missing_required_features"][0]["feature"],"order_book_depth");
    }

    #[test]
    fn ingested_v1_vector_runs_real_engine_and_returns_attribution_when_complete(){
        let mut vector=MarketVector::new(
            "ingest-v1-ready","test-venue","BTCUSDT","event-window",
            1_800_000_000_000,1_800_000_000_001,
        ).unwrap();
        for feature in [
            MarketFeature::OrderFlowImbalance,MarketFeature::OrderBookDepth,
            MarketFeature::RealizedVolatility,MarketFeature::FundingPressure,
            MarketFeature::OpenInterestChange,MarketFeature::LiquidationPressure,
            MarketFeature::RegimeVolatilityLevel,
            MarketFeature::RegimeLiquidityStress,MarketFeature::RegimeCorrelationStress,
            MarketFeature::RegimeTransitionIntensity,MarketFeature::RegimeUncertainty,
        ]{
            vector.set_value(feature,Some(0.0),0.95).unwrap();
        }
        vector.set_not_applicable(MarketFeature::Basis);
        vector.set_value(MarketFeature::OrderFlowImbalance,Some(0.9),0.95).unwrap();
        let result=evaluate_ingested_v1_vector(&vector,&EngineConfig::default(),"v1-test");
        assert_eq!(result["status"],"evaluated");
        assert_eq!(result["decision"]["attribution"]["method"],"exact_shapley");
        assert_eq!(result["decision"]["attribution"]["contributions"].as_array().unwrap().len(),12);
    }



    #[test]
    fn v1_attribution_is_throttled_to_one_evaluation_per_second(){
        assert!(should_evaluate_v1(None,1_000));
        assert!(!should_evaluate_v1(Some(1_001),1_999));
        assert!(should_evaluate_v1(Some(1_001),2_000));
        assert!(!should_evaluate_v1(Some(2_001),2_999));
    }

    #[test]
    fn v2_history_discovery_is_throttled_to_new_five_minute_buckets(){
        assert!(!should_run_v2_history_discovery(None,300_001));
        assert!(!should_run_v2_history_discovery(Some(300_001),599_999));
        assert!(should_run_v2_history_discovery(Some(299_999),300_000));
        assert!(should_run_v2_history_discovery(Some(300_001),600_000));
    }

    #[test]
    fn recovery_preserves_late_event_exclusion_and_legacy_event_compatibility(){
        let late=doc!{"_qoin_applied":false,"_qoin_pipeline_status":"ignored_late"};
        let applied=doc!{"_qoin_applied":true,"_qoin_pipeline_status":"complete"};
        let pending=doc!{"_qoin_applied":true,"_qoin_pipeline_status":"pending"};
        let legacy=doc!{"event_id":"legacy-event"};
        assert!(!persisted_event_is_applied(&late));
        assert!(persisted_event_is_applied(&applied));
        assert!(persisted_event_is_applied(&legacy));
        assert!(pending_pipeline_event(&pending));
        assert!(!pending_pipeline_event(&applied));
    }
}

#[cfg(test)]
mod execution_route_auth_tests {
    use super::*;

    #[test]
    fn persistent_variable_hub_round_trips_through_mongodb_bson() {
        let mut hub=VariableHub::new("binance_usdm","BTCUSDT","5m").unwrap();
        hub.observe_input(MarketFeature::LogReturn,0.001,0.95,
            1_800_000_000_000,1_800_000_000_001,"trade-feed","log-return-v1").unwrap();
        let document=mongodb::bson::to_document(&hub).unwrap();
        let restored=mongodb::bson::from_document::<VariableHub>(document).unwrap();
        assert_eq!(restored.identity.venue,"binance_usdm");
        assert_eq!(restored.progress(MarketFeature::LogReturn).lifecycle,
            qoin_market_space::hub::VariableLifecycle::Observed);
        assert_eq!(restored.value(MarketFeature::LogReturn).unwrap().raw_value,Some(0.001));
    }

    #[test]
    fn wall_clock_gate_rejects_future_market_availability(){
        let now=chrono::Utc::now().timestamp_millis();
        assert!(timestamps_are_not_future(now-1,now));
        assert!(!timestamps_are_not_future(now,now+1));
        let future=MarketEvent{
            event_id:"future".into(),venue:"test".into(),symbol:"BTCUSDT".into(),
            event_type:"trade".into(),ts_ms:now+1,price:Some(100.0),size:Some(1.0),
            side:Some("buy".into()),bid:None,ask:None,bid_size:None,ask_size:None,
            funding_rate:None,open_interest:None,mark_price:None,index_price:None,liquidation_side:None,
        };
        assert!(validate_event(&future).is_err());
    }

    #[test]
    fn accepts_only_exact_execution_tokens() {
        assert!(constant_time_token_match(b"operator-token-123", b"operator-token-123"));
        assert!(!constant_time_token_match(b"operator-token-123", b"operator-token-124"));
        assert!(!constant_time_token_match(b"operator-token-123", b"operator-token-1234"));
        assert!(!constant_time_token_match(b"operator-token-123", b""));
    }
}
