//! Deterministic multi-asset portfolio accounting and research primitives.
//! Inputs are explicit, timestamped observations. This crate never places orders.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum ResearchError {
    #[error("invalid portfolio input: {0}")] Invalid(String),
    #[error("missing FX conversion from {0} to base currency")] MissingFx(String),
    #[error("correlation matrix is invalid: {0}")] Correlation(String),
    #[error("insufficient observations for requested evaluation")] InsufficientData,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentSpec {
    pub instrument_id: String,
    pub asset_class: String,
    pub quote_currency: String,
    pub contract_multiplier: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LedgerEvent {
    Deposit { ts_ms: i64, currency: String, amount: f64 },
    Withdrawal { ts_ms: i64, currency: String, amount: f64 },
    Trade { ts_ms: i64, instrument_id: String, quantity_delta: f64, price: f64, fee: f64, fee_currency: String },
    Funding { ts_ms: i64, instrument_id: String, amount: f64, currency: String },
    Borrow { ts_ms: i64, currency: String, amount: f64 },
    Interest { ts_ms: i64, currency: String, amount: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxRate { pub currency: String, pub base_per_unit: f64, pub ts_ms: i64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mark { pub ts_ms: i64, pub instrument_id: String, pub price: f64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EquityPoint { pub ts_ms: i64, pub equity_base: f64, pub regime: Option<String> }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CorrelationPair { pub left: String, pub right: String, pub correlation: f64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskLimits {
    pub max_instrument_exposure_pct: f64,
    pub max_asset_class_exposure_pct: f64,
    pub max_correlated_cluster_exposure_pct: f64,
    pub max_drawdown_pct: f64,
    pub max_gross_leverage: f64,
    pub max_single_trade_pct: f64,
}

impl Default for RiskLimits {
    fn default() -> Self { Self { max_instrument_exposure_pct: 25.0, max_asset_class_exposure_pct: 60.0,
        max_correlated_cluster_exposure_pct: 40.0, max_drawdown_pct: 20.0, max_gross_leverage: 1.0,
        max_single_trade_pct: 10.0 } }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioInput {
    pub base_currency: String,
    pub instruments: Vec<InstrumentSpec>,
    pub events: Vec<LedgerEvent>,
    pub fx_rates: Vec<FxRate>,
    pub marks: Vec<Mark>,
    pub equity_curve: Vec<EquityPoint>,
    pub correlations: Vec<CorrelationPair>,
    pub risk_limits: RiskLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub instrument_id: String,
    pub quantity: f64,
    pub mark_price: f64,
    pub market_value_quote: f64,
    pub market_value_base: f64,
    pub realized_pnl_quote: f64,
    pub fees_quote: f64,
    pub asset_class: String,
    pub quote_currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RiskViolation { pub code: String, pub subject: String, pub observed: f64, pub limit: f64 }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PortfolioMetrics {
    pub total_return_pct: f64,
    pub annualized_volatility_pct: Option<f64>,
    pub sharpe_ratio: Option<f64>,
    pub sortino_ratio: Option<f64>,
    pub calmar_ratio: Option<f64>,
    pub max_drawdown_pct: f64,
    pub turnover_pct: f64,
    pub average_holding_hours: Option<f64>,
    pub observations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioReport {
    pub run_fingerprint: String,
    pub base_currency: String,
    pub cash_balances: BTreeMap<String, f64>,
    pub cash_base_value: f64,
    pub positions: Vec<Position>,
    pub gross_exposure_base: f64,
    pub net_exposure_base: f64,
    pub equity_base: f64,
    pub realized_pnl_base: f64,
    pub fees_base: f64,
    pub funding_base: f64,
    pub borrow_base: f64,
    pub risk_violations: Vec<RiskViolation>,
    pub metrics: PortfolioMetrics,
    pub regime_metrics: BTreeMap<String, PortfolioMetrics>,
    pub assumptions: Vec<String>,
}

fn finite(v: f64) -> bool { v.is_finite() }
fn validate(input: &PortfolioInput) -> Result<(), ResearchError> {
    if input.base_currency.trim().is_empty() { return Err(ResearchError::Invalid("base currency is required".into())); }
    if input.risk_limits.max_drawdown_pct <= 0.0 || input.risk_limits.max_drawdown_pct > 100.0
        || input.risk_limits.max_gross_leverage <= 0.0 {
        return Err(ResearchError::Invalid("risk limits must be positive and drawdown <= 100%".into()));
    }
    let mut ids = BTreeSet::new();
    for i in &input.instruments {
        if i.instrument_id.trim().is_empty() || i.quote_currency.trim().is_empty()
            || !finite(i.contract_multiplier) || i.contract_multiplier <= 0.0 || !ids.insert(i.instrument_id.clone()) {
            return Err(ResearchError::Invalid("instrument identity, multiplier or uniqueness is invalid".into()));
        }
    }
    for e in &input.events {
        let (ts, amount) = match e {
            LedgerEvent::Deposit{ts_ms,amount,..}|LedgerEvent::Withdrawal{ts_ms,amount,..}
            |LedgerEvent::Borrow{ts_ms,amount,..}|LedgerEvent::Interest{ts_ms,amount,..}
            |LedgerEvent::Funding{ts_ms,amount,..} => (*ts_ms,*amount),
            LedgerEvent::Trade{ts_ms,quantity_delta,price,fee,..} => {
                if !finite(*price) || *price <= 0.0 || !finite(*quantity_delta) || !finite(*fee) || *fee < 0.0 {
                    return Err(ResearchError::Invalid("trade price, quantity or fee is invalid".into()));
                }
                (*ts_ms,*quantity_delta)
            }
        };
        if ts <= 0 || !finite(amount) { return Err(ResearchError::Invalid("ledger timestamps and amounts must be finite and positive timestamps".into())); }
    }
    for fx in &input.fx_rates {
        if fx.currency.trim().is_empty() || fx.ts_ms <= 0 || !finite(fx.base_per_unit) || fx.base_per_unit <= 0.0 {
            return Err(ResearchError::Invalid("FX rates require positive finite values and timestamps".into()));
        }
    }
    for m in &input.marks {
        if m.ts_ms <= 0 || !finite(m.price) || m.price <= 0.0 { return Err(ResearchError::Invalid("marks must have positive finite prices and timestamps".into())); }
    }
    for c in &input.correlations {
        if c.left == c.right || !finite(c.correlation) || !(-1.0..=1.0).contains(&c.correlation) {
            return Err(ResearchError::Correlation("pairs must be distinct and correlations in [-1,1]".into()));
        }
    }
    Ok(())
}

fn fx_to_base(currency: &str, base: &str, rates: &[FxRate], at: i64) -> Result<f64, ResearchError> {
    if currency == base { return Ok(1.0); }
    rates.iter().filter(|r| r.currency == currency && r.ts_ms <= at)
        .max_by_key(|r| r.ts_ms).map(|r| r.base_per_unit)
        .ok_or_else(|| ResearchError::MissingFx(currency.to_owned()))
}

fn metric(curve: &[EquityPoint], turnover_pct: f64, holding_hours: Option<f64>) -> PortfolioMetrics {
    if curve.len() < 2 {
        return PortfolioMetrics { total_return_pct: 0.0, annualized_volatility_pct: None, sharpe_ratio: None,
            sortino_ratio: None, calmar_ratio: None, max_drawdown_pct: 0.0, turnover_pct,
            average_holding_hours: holding_hours, observations: curve.len() };
    }
    let first = curve[0].equity_base;
    let last = curve[curve.len()-1].equity_base;
    let returns: Vec<f64> = curve.windows(2).filter_map(|w| {
        (w[0].equity_base > 0.0).then_some(w[1].equity_base / w[0].equity_base - 1.0)
    }).collect();
    let mean = returns.iter().sum::<f64>() / returns.len().max(1) as f64;
    let variance = if returns.len() > 1 { returns.iter().map(|r|(r-mean).powi(2)).sum::<f64>()/(returns.len()-1) as f64 } else { 0.0 };
    let downside: Vec<f64> = returns.iter().copied().filter(|r| *r < 0.0).collect();
    let downside_dev = if downside.is_empty() { 0.0 } else {
        (downside.iter().map(|r|r*r).sum::<f64>()/returns.len() as f64).sqrt()
    };
    let elapsed_years = (curve.last().unwrap().ts_ms - curve[0].ts_ms).max(1) as f64 / (365.25*24.0*3600.0*1000.0);
    let periods_per_year = (returns.len() as f64 / elapsed_years).max(1.0);
    let vol = variance.sqrt() * periods_per_year.sqrt();
    let annual_return = if first > 0.0 && last > 0.0 { (last/first).powf(1.0/elapsed_years)-1.0 } else { 0.0 };
    let mut peak = curve[0].equity_base;
    let mut max_dd: f64 = 0.0;
    for p in curve { peak = peak.max(p.equity_base); if peak > 0.0 { max_dd = max_dd.max((peak-p.equity_base)/peak); } }
    PortfolioMetrics {
        total_return_pct: if first > 0.0 {(last/first-1.0)*100.0} else {0.0},
        annualized_volatility_pct: Some(vol*100.0),
        sharpe_ratio: (variance > 0.0).then_some(mean/variance.sqrt()*periods_per_year.sqrt()),
        sortino_ratio: (downside_dev > 0.0).then_some(mean/downside_dev*periods_per_year.sqrt()),
        calmar_ratio: (max_dd > 0.0).then_some(annual_return/max_dd),
        max_drawdown_pct: max_dd*100.0, turnover_pct, average_holding_hours: holding_hours, observations: curve.len()
    }
}

/// Deterministic ledger valuation. Marks and FX are point-in-time as-of the latest supplied ledger-event or mark timestamp.
pub fn evaluate_portfolio(input: &PortfolioInput) -> Result<PortfolioReport, ResearchError> {
    validate(input)?;
    if input.instruments.is_empty() { return Err(ResearchError::Invalid("at least one instrument is required".into())); }
    let mut events=input.events.clone();
    events.sort_by_key(|e| match e {
        LedgerEvent::Deposit{ts_ms,..}|LedgerEvent::Withdrawal{ts_ms,..}|LedgerEvent::Trade{ts_ms,..}
        |LedgerEvent::Funding{ts_ms,..}|LedgerEvent::Borrow{ts_ms,..}|LedgerEvent::Interest{ts_ms,..}=>*ts_ms
    });
    // Value the ledger at the latest supplied event/mark timestamp. A mark
    // recorded after the last cash or trade event is still the correct
    // valuation observation; restricting the clock to the last ledger event
    // silently falls back to entry price and understates/overstates equity.
    let last_event_ts=events.last().map(|e| match e {
        LedgerEvent::Deposit{ts_ms,..}|LedgerEvent::Withdrawal{ts_ms,..}|LedgerEvent::Trade{ts_ms,..}
        |LedgerEvent::Funding{ts_ms,..}|LedgerEvent::Borrow{ts_ms,..}|LedgerEvent::Interest{ts_ms,..}=>*ts_ms
    }).unwrap_or(0);
    let last_mark_ts=input.marks.iter().map(|m|m.ts_ms).max().unwrap_or(0);
    let end_ts=last_event_ts.max(last_mark_ts).max(1);
    let specs: BTreeMap<_,_>=input.instruments.iter().map(|i|(i.instrument_id.as_str(),i)).collect();
    let mut cash:BTreeMap<String,f64>=BTreeMap::new();
    let mut qty:BTreeMap<String,f64>=BTreeMap::new();
    let mut avg:BTreeMap<String,f64>=BTreeMap::new();
    let mut realized_quote:BTreeMap<String,f64>=BTreeMap::new();
    let mut fees_quote:BTreeMap<String,f64>=BTreeMap::new();
    let mut funding_base=0.0; let mut borrow_base=0.0; let mut fees_base=0.0; let mut turnover=0.0;
    let mut hold_starts:BTreeMap<String,i64>=BTreeMap::new(); let mut holding_ms=Vec::new();
    for e in &events {
        match e {
            LedgerEvent::Deposit{currency,amount,..}=>*cash.entry(currency.clone()).or_default()+=amount,
            LedgerEvent::Withdrawal{currency,amount,..}=>*cash.entry(currency.clone()).or_default()-=amount,
            LedgerEvent::Borrow{currency,amount,..}=>{*cash.entry(currency.clone()).or_default()+=amount;borrow_base+=amount*fx_to_base(currency,&input.base_currency,&input.fx_rates,end_ts)?;},
            LedgerEvent::Interest{currency,amount,..}=>*cash.entry(currency.clone()).or_default()+=amount,
            LedgerEvent::Funding{instrument_id,amount,currency,..}=>{
                *cash.entry(currency.clone()).or_default()+=amount;
                funding_base+=amount*fx_to_base(currency,&input.base_currency,&input.fx_rates,end_ts)?;
                let _=instrument_id;
            },
            LedgerEvent::Trade{ts_ms,instrument_id,quantity_delta,price,fee,fee_currency}=>{
                let spec=specs.get(instrument_id.as_str()).ok_or_else(||ResearchError::Invalid(format!("unknown instrument {instrument_id}")))?;
                let old=*qty.get(instrument_id).unwrap_or(&0.0);
                let new=old+quantity_delta;
                let old_avg=*avg.get(instrument_id).unwrap_or(&0.0);
                if old==0.0 && new!=0.0 {hold_starts.insert(instrument_id.clone(),*ts_ms);}
                if old!=0.0 && (new==0.0 || old.signum()!=new.signum()) {
                    if let Some(start)=hold_starts.remove(instrument_id) {holding_ms.push(ts_ms.saturating_sub(start).max(0) as f64);}
                    if new!=0.0 {hold_starts.insert(instrument_id.clone(),*ts_ms);}
                }
                let closing=if old.signum()!=quantity_delta.signum() {old.abs().min(quantity_delta.abs())} else {0.0};
                let pnl=closing*(price-old_avg)*old.signum()*spec.contract_multiplier;
                *realized_quote.entry(instrument_id.clone()).or_default()+=pnl;
                let opening=quantity_delta.abs()-closing;
                let next_avg=if new==0.0 {0.0} else if old==0.0 || old.signum()!=new.signum() { *price }
                    else if old.signum()==quantity_delta.signum() { (old.abs()*old_avg+quantity_delta.abs()*price)/new.abs() }
                    else {old_avg};
                qty.insert(instrument_id.clone(),new); avg.insert(instrument_id.clone(),next_avg);
                let notional=quantity_delta.abs()*price*spec.contract_multiplier;
                turnover+=notional*fx_to_base(&spec.quote_currency,&input.base_currency,&input.fx_rates,*ts_ms)?;
                *cash.entry(spec.quote_currency.clone()).or_default()-=quantity_delta*price*spec.contract_multiplier;
                *cash.entry(fee_currency.clone()).or_default()-=*fee;
                *fees_quote.entry(instrument_id.clone()).or_default()+=fee;
                fees_base+=fee*fx_to_base(fee_currency,&input.base_currency,&input.fx_rates,*ts_ms)?;
                let _=opening;
            }
        }
    }
    let mut positions=Vec::new(); let mut gross=0.0; let mut net=0.0; let mut realized_base=0.0;
    let mut class_exposure:BTreeMap<String,f64>=BTreeMap::new(); let mut instrument_exposure:BTreeMap<String,f64>=BTreeMap::new();
    for (id,q) in &qty {
        let spec=specs[id.as_str()];
        let mark=input.marks.iter().filter(|m|m.instrument_id==*id && m.ts_ms<=end_ts).max_by_key(|m|m.ts_ms)
            .map(|m|m.price).unwrap_or(*avg.get(id).unwrap_or(&0.0));
        let quote_value=q*mark*spec.contract_multiplier;
        let fx=fx_to_base(&spec.quote_currency,&input.base_currency,&input.fx_rates,end_ts)?;
        let base_value=quote_value*fx;
        gross+=base_value.abs(); net+=base_value;
        *class_exposure.entry(spec.asset_class.clone()).or_default()+=base_value.abs();
        instrument_exposure.insert(id.clone(),base_value.abs());
        realized_base+=realized_quote.get(id).copied().unwrap_or(0.0)*fx;
        positions.push(Position{instrument_id:id.clone(),quantity:*q,mark_price:mark,market_value_quote:quote_value,
            market_value_base:base_value,realized_pnl_quote:realized_quote.get(id).copied().unwrap_or(0.0),
            fees_quote:fees_quote.get(id).copied().unwrap_or(0.0),asset_class:spec.asset_class.clone(),quote_currency:spec.quote_currency.clone()});
    }
    let mut cash_base=0.0;
    for (currency,amount) in &cash {cash_base+=amount*fx_to_base(currency,&input.base_currency,&input.fx_rates,end_ts)?;}
    let equity=cash_base+net;
    if !finite(equity) || equity<=0.0 {return Err(ResearchError::Invalid("portfolio equity must be positive and finite".into()));}
    let mut violations=Vec::new();
    let limits=&input.risk_limits;
    let add=|code:&str,subject:String,observed:f64,limit:f64,out:&mut Vec<RiskViolation>|{if observed>limit {out.push(RiskViolation{code:code.into(),subject,observed,limit});}};
    for (id,value) in &instrument_exposure {add("instrument_exposure",id.clone(),value/equity*100.0,limits.max_instrument_exposure_pct,&mut violations);}
    for (class,value) in &class_exposure {add("asset_class_exposure",class.clone(),value/equity*100.0,limits.max_asset_class_exposure_pct,&mut violations);}
    add("gross_leverage","portfolio".into(),gross/equity,limits.max_gross_leverage,&mut violations);
    let mut seen=BTreeSet::new();
    for pair in &input.correlations {
        if pair.correlation>=0.8 {
            let key=if pair.left<pair.right { (pair.left.clone(),pair.right.clone()) } else {(pair.right.clone(),pair.left.clone())};
            if seen.insert(key) {
                let exposure=instrument_exposure.get(&pair.left).copied().unwrap_or(0.0)+instrument_exposure.get(&pair.right).copied().unwrap_or(0.0);
                add("correlated_cluster_exposure",format!("{}+{}",pair.left,pair.right),exposure/equity*100.0,limits.max_correlated_cluster_exposure_pct,&mut violations);
            }
        }
    }
    let mut curve=input.equity_curve.clone(); curve.sort_by_key(|p|p.ts_ms);
    if curve.iter().any(|p|p.ts_ms<=0 || !finite(p.equity_base) || p.equity_base<=0.0) {return Err(ResearchError::Invalid("equity curve points must be positive and finite".into()));}
    let total_turnover=if equity>0.0 {turnover/equity*100.0}else{0.0};
    let avg_hold=(!holding_ms.is_empty()).then_some(holding_ms.iter().sum::<f64>()/holding_ms.len() as f64/3_600_000.0);
    let metrics=metric(&curve,total_turnover,avg_hold);
    add("max_drawdown","portfolio".into(),metrics.max_drawdown_pct,limits.max_drawdown_pct,&mut violations);
    let mut regimes:BTreeMap<String,Vec<EquityPoint>>=BTreeMap::new();
    for point in &curve {if let Some(regime)=&point.regime {regimes.entry(regime.clone()).or_default().push(point.clone());}}
    let regime_metrics=regimes.into_iter().map(|(name,points)|(name,metric(&points,total_turnover,None))).collect();
    let canonical=serde_json::to_vec(input).map_err(|e|ResearchError::Invalid(e.to_string()))?;
    let fingerprint=format!("{:x}",Sha256::digest(canonical));
    Ok(PortfolioReport{run_fingerprint:fingerprint,base_currency:input.base_currency.clone(),cash_balances:cash,
        cash_base_value:cash_base,positions,gross_exposure_base:gross,net_exposure_base:net,equity_base:equity,
        realized_pnl_base:realized_base,fees_base,funding_base,borrow_base,risk_violations:violations,metrics,
        regime_metrics,assumptions:vec!["All amounts are valued in the declared base currency using the latest supplied FX rate at or before the valuation timestamp.".into(),
        "Missing instrument marks fall back to weighted average entry price; this is not a live executable quote.".into(),
        "Correlation limits use supplied pairwise correlations and pair sums; they are not a full covariance or marginal VaR model.".into(),
        "Sharpe and Sortino annualization assumes equally spaced equity observations; supply a regular sampling interval.".into(),
        "Walk-forward and out-of-sample evaluation must use chronologically separated input windows.".into()]})
}


#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionSizingInput {
    pub equity_base: f64,
    pub entry_price_quote: f64,
    pub stop_price_quote: f64,
    pub quote_to_base: f64,
    pub contract_multiplier: f64,
    pub risk_budget_pct: f64,
    pub max_notional_pct: f64,
    pub existing_instrument_exposure_base: f64,
    pub max_gross_leverage: f64,
    pub current_gross_exposure_base: f64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionSize {
    pub quantity: f64,
    pub notional_base: f64,
    pub loss_at_stop_base: f64,
    pub risk_budget_base: f64,
    pub binding_constraint: String,
}
/// Conservative long/short sizing from stop distance, portfolio risk budget,
/// instrument concentration and remaining gross leverage. No order is submitted.
pub fn size_position(input: &PositionSizingInput) -> Result<PositionSize, ResearchError> {
    let values = [input.equity_base,input.entry_price_quote,input.stop_price_quote,input.quote_to_base,
        input.contract_multiplier,input.risk_budget_pct,input.max_notional_pct,
        input.existing_instrument_exposure_base,input.max_gross_leverage,input.current_gross_exposure_base];
    if values.iter().any(|v| !finite(*v)) || input.equity_base<=0.0 || input.entry_price_quote<=0.0
        || input.stop_price_quote<=0.0 || input.quote_to_base<=0.0 || input.contract_multiplier<=0.0
        || input.risk_budget_pct<=0.0 || input.risk_budget_pct>100.0 || input.max_notional_pct<=0.0
        || input.max_notional_pct>100.0 || input.existing_instrument_exposure_base<0.0
        || input.max_gross_leverage<=0.0 || input.current_gross_exposure_base<0.0 {
        return Err(ResearchError::Invalid("position sizing inputs and risk budgets must be finite and within valid bounds".into()));
    }
    let stop_distance=(input.entry_price_quote-input.stop_price_quote).abs();
    if stop_distance<=0.0 {return Err(ResearchError::Invalid("entry and stop prices must differ".into()));}
    let risk_budget=input.equity_base*input.risk_budget_pct/100.0;
    let risk_per_unit=stop_distance*input.quote_to_base*input.contract_multiplier;
    let quantity_by_risk=risk_budget/risk_per_unit;
    let max_instrument=input.equity_base*input.max_notional_pct/100.0;
    let remaining_instrument=(max_instrument-input.existing_instrument_exposure_base).max(0.0);
    let remaining_gross=(input.equity_base*input.max_gross_leverage-input.current_gross_exposure_base).max(0.0);
    let max_notional=remaining_instrument.min(remaining_gross);
    let quantity_by_notional=max_notional/(input.entry_price_quote*input.quote_to_base*input.contract_multiplier);
    let quantity=quantity_by_risk.min(quantity_by_notional);
    if !finite(quantity)||quantity<=0.0 {return Err(ResearchError::Invalid("risk or exposure limits leave no position capacity".into()));}
    let notional=quantity*input.entry_price_quote*input.quote_to_base*input.contract_multiplier;
    let loss=quantity*risk_per_unit;
    let binding=if quantity_by_risk<=quantity_by_notional {"stop_loss_risk_budget"} else if remaining_instrument<=remaining_gross {"instrument_concentration"} else {"gross_leverage"};
    Ok(PositionSize{quantity,notional_base:notional,loss_at_stop_base:loss,risk_budget_base:risk_budget,binding_constraint:binding.into()})
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationWindow { pub train_start_ms:i64, pub train_end_ms:i64, pub test_start_ms:i64, pub test_end_ms:i64 }
pub fn walk_forward_windows(start_ms:i64,end_ms:i64,train_days:u32,test_days:u32,step_days:u32)->Result<Vec<EvaluationWindow>,ResearchError>{
    if start_ms<=0 || end_ms<=start_ms || train_days==0 || test_days==0 || step_days==0 {return Err(ResearchError::Invalid("walk-forward dates and day counts must be positive".into()));}
    let day=86_400_000_i64; let train=train_days as i64*day; let test=test_days as i64*day; let step=step_days as i64*day;
    let mut windows=Vec::new(); let mut train_start=start_ms;
    while train_start.saturating_add(train).saturating_add(test)<=end_ms {
        let train_end=train_start+train; windows.push(EvaluationWindow{train_start_ms:train_start,train_end_ms:train_end,test_start_ms:train_end,test_end_ms:train_end+test});
        train_start=train_start.saturating_add(step);
    }
    if windows.is_empty(){return Err(ResearchError::InsufficientData);} Ok(windows)
}

#[cfg(test)]
mod tests {
 use super::*;
 fn sample()->PortfolioInput{PortfolioInput{base_currency:"USD".into(),instruments:vec![
  InstrumentSpec{instrument_id:"BTC-PERP".into(),asset_class:"crypto".into(),quote_currency:"USD".into(),contract_multiplier:1.0},
  InstrumentSpec{instrument_id:"EURUSD".into(),asset_class:"forex".into(),quote_currency:"USD".into(),contract_multiplier:1000.0}],
  events:vec![LedgerEvent::Deposit{ts_ms:1_000,currency:"USD".into(),amount:10_000.0},
   LedgerEvent::Trade{ts_ms:2_000,instrument_id:"BTC-PERP".into(),quantity_delta:0.1,price:50_000.0,fee:2.0,fee_currency:"USD".into()},
   LedgerEvent::Funding{ts_ms:3_000,instrument_id:"BTC-PERP".into(),amount:1.0,currency:"USD".into()}],
  fx_rates:vec![],marks:vec![Mark{ts_ms:4_000,instrument_id:"BTC-PERP".into(),price:51_000.0}],
  equity_curve:vec![EquityPoint{ts_ms:1_000,equity_base:10_000.0,regime:Some("trend".into())},EquityPoint{ts_ms:2_000,equity_base:10_100.0,regime:Some("trend".into())},EquityPoint{ts_ms:3_000,equity_base:9_900.0,regime:Some("volatile".into())}],
  correlations:vec![],risk_limits:RiskLimits::default()}}
 #[test] fn multi_asset_ledger_values_cash_position_and_funding(){let r=evaluate_portfolio(&sample()).unwrap();assert_eq!(r.positions.len(),1);assert!((r.cash_balances["USD"]-4999.0).abs()<1e-8);assert!((r.equity_base-10099.0).abs()<1e-8);assert_eq!(r.funding_base,1.0);}
 #[test] fn non_base_currency_requires_asof_fx(){let mut x=sample();x.events.push(LedgerEvent::Deposit{ts_ms:5_000,currency:"EUR".into(),amount:100.0});assert!(matches!(evaluate_portfolio(&x),Err(ResearchError::MissingFx(_))));}
 #[test] fn risk_limits_report_instrument_and_gross_exposure(){let mut x=sample();x.risk_limits.max_instrument_exposure_pct=10.0;let r=evaluate_portfolio(&x).unwrap();assert!(r.risk_violations.iter().any(|v|v.code=="instrument_exposure"));}
 #[test] fn walk_forward_windows_are_chronological_and_non_overlapping(){let d=86_400_000;let w=walk_forward_windows(1_000,1_000+100*d,60,20,20).unwrap();assert_eq!(w.len(),2);assert_eq!(w[0].train_end_ms,w[0].test_start_ms);assert_eq!(w[0].test_end_ms,w[1].test_start_ms);}
 #[test] fn identical_input_has_identical_fingerprint(){let x=sample();assert_eq!(evaluate_portfolio(&x).unwrap().run_fingerprint,evaluate_portfolio(&x).unwrap().run_fingerprint);}
}
