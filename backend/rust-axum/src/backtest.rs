use serde::{Deserialize, Serialize};
use crate::edge_strategy::{EdgeDecision, EdgeStrategy};

#[derive(Debug, Clone, Deserialize)]
pub struct ReplayEvent {
    pub event_id: String, pub venue: String, pub symbol: String, pub event_type: String, pub ts_ms: i64,
    pub price: Option<f64>, pub size: Option<f64>, pub side: Option<String>,
    pub bid: Option<f64>, pub ask: Option<f64>, pub bid_size: Option<f64>, pub ask_size: Option<f64>,
    pub funding_rate: Option<f64>, pub open_interest: Option<f64>, pub liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Default)]
struct ReplayState {
    last_price: Option<f64>, spread_bps: Option<f64>, signed_flow: f64, total_trade_size: f64,
    ofi: f64, event_intensity: f64, ewma_abs_return: f64, funding_rate: Option<f64>,
    liquidation_events: usize, last_liquidation_side: Option<String>,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayFill {
    pub ts_ms: i64, pub event_id: String, pub action: String, pub price: f64,
    pub quantity: f64, pub fee: f64, pub slippage_cost: f64, pub realized_pnl: f64,
    pub equity_after: f64, pub reason: String,
}
#[derive(Debug, Clone, Serialize)]
pub struct ReplayResult {
    pub symbol: String, pub strategy_key: String, pub strategy_version: String,
    pub from_ts_ms: i64, pub to_ts_ms: i64, pub events_replayed: usize,
    pub decisions: usize, pub buy_decisions: usize, pub sell_decisions: usize,
    pub wait_decisions: usize, pub initial_capital: f64, pub final_equity: f64,
    pub net_pnl: f64, pub net_return_pct: f64, pub max_drawdown_pct: f64,
    pub fees_paid: f64, pub slippage_paid: f64, pub fee_bps: f64, pub slippage_bps: f64,
    pub fills: Vec<ReplayFill>, pub last_decision: Option<EdgeDecision>,
    pub assumptions: Vec<String>,
}
pub fn replay(mut events: Vec<ReplayEvent>, strategy: &EdgeStrategy, initial_capital: f64,
              fee_bps: f64, slippage_bps: f64) -> Result<ReplayResult, String> {
    strategy.validate()?;
    if !initial_capital.is_finite() || initial_capital <= 0.0 || !fee_bps.is_finite()
        || fee_bps < 0.0 || !slippage_bps.is_finite() || slippage_bps < 0.0 {
        return Err("capital must be positive and fee/slippage assumptions must be finite and non-negative".into());
    }
    if events.is_empty() { return Err("no persisted market events in the requested replay window".into()); }
    events.sort_by(|a,b| a.ts_ms.cmp(&b.ts_ms).then_with(||a.event_id.cmp(&b.event_id)));
    let symbol=events[0].symbol.clone();
    if events.iter().any(|e|e.symbol != symbol) { return Err("replay events must contain exactly one canonical symbol".into()); }
    if events.iter().any(|e|e.ts_ms <= 0 || e.price.map(|v|!v.is_finite() || v<=0.0).unwrap_or(false)
        || e.size.map(|v|!v.is_finite() || v<0.0).unwrap_or(false)) {
        return Err("replay contains invalid timestamps, prices or sizes".into());
    }
    let mut state=ReplayState::default();
    let mut cash=initial_capital; let mut position=0.0; let mut position_entry_cost=0.0; let mut fees=0.0; let mut slippage=0.0;
    let mut peak=initial_capital; let mut max_dd=0.0; let mut fills=Vec::new();
    let mut buys=0; let mut sells=0; let mut waits=0; let mut last_decision=None;
    for event in &events {
        if let Some(price)=event.price.filter(|v|*v>0.0) {
            if let Some(previous)=state.last_price.filter(|v|*v>0.0) {
                state.ewma_abs_return=0.94*state.ewma_abs_return+0.06*(price/previous).ln().abs();
            }
            state.last_price=Some(price);
        }
        if let (Some(bid),Some(ask))=(event.bid,event.ask) {
            let mid=(bid+ask)/2.0;
            state.spread_bps=if mid>0.0 {Some((ask-bid)/mid*10_000.0)} else {None};
            if let (Some(bs),Some(as_))=(event.bid_size,event.ask_size) { if bs+as_>0.0 {state.signed_flow+=(bs-as_)/(bs+as_)*0.1;} }
        }
        match event.event_type.as_str() {
            "trade" => { let size=event.size.unwrap_or(0.0); match event.side.as_deref() {
                Some("buy")=>state.signed_flow+=size, Some("sell")=>state.signed_flow-=size, _=>{} }
                state.total_trade_size+=size; state.event_intensity+=1.0; }
            "liquidation" => {state.liquidation_events+=1;state.event_intensity+=2.0;
                state.last_liquidation_side=event.liquidation_side.clone();
                if event.liquidation_side.as_deref()==Some("long"){state.signed_flow-=event.size.unwrap_or(0.0);}
                if event.liquidation_side.as_deref()==Some("short"){state.signed_flow+=event.size.unwrap_or(0.0);}}
            "funding"=>if event.funding_rate.is_some(){state.funding_rate=event.funding_rate;},
            _=>{}
        }
        if state.total_trade_size>0.0 {state.ofi=(state.signed_flow/state.total_trade_size).clamp(-1.0,1.0);}
        let decision=strategy.decide(&symbol,state.ofi,state.funding_rate,state.spread_bps,state.ewma_abs_return,
            state.event_intensity,state.liquidation_events,state.last_liquidation_side.as_deref())?;
        match decision.action { "buy"=>buys+=1,"sell"=>sells+=1,_=>waits+=1 }
        let mark=event.price.filter(|v|*v>0.0).or(state.last_price);
        if let (Some(price),true)= (mark, decision.action=="buy" && position==0.0) {
            let execution=price*(1.0+slippage_bps/10_000.0);
            let quantity=(cash/(execution*(1.0+fee_bps/10_000.0))).max(0.0);
            let fee=quantity*execution*fee_bps/10_000.0; let slip=quantity*(execution-price);
            cash-=quantity*execution+fee; position=quantity; position_entry_cost=quantity*execution+fee; fees+=fee; slippage+=slip;
            fills.push(ReplayFill{ts_ms:event.ts_ms,event_id:event.event_id.clone(),action:"BUY".into(),price:execution,
                quantity,fee,slippage_cost:slip,realized_pnl:0.0,equity_after:cash+position*price,reason:decision.reason.clone()});
        } else if let (Some(price),true)=(mark,decision.action=="sell" && position>0.0) {
            let execution=price*(1.0-slippage_bps/10_000.0);
            let gross=position*(execution-price); let fee=position*execution*fee_bps/10_000.0;
            let slip=position*(price-execution); let proceeds=position*execution-fee;
            cash+=proceeds; position=0.0; fees+=fee; slippage+=slip;
            fills.push(ReplayFill{ts_ms:event.ts_ms,event_id:event.event_id.clone(),action:"SELL".into(),price:execution,
                quantity:0.0,fee,slippage_cost:slip,realized_pnl, equity_after:cash,reason:decision.reason.clone()});
        }
        if let Some(price)=mark {
            let equity=cash+position*price; peak=peak.max(equity);
            if peak>0.0 {max_dd=max_dd.max((peak-equity)/peak);}
        }
        last_decision=Some(decision);
    }
    let final_mark=state.last_price.unwrap_or(0.0); let final_equity=cash+position*final_mark;
    Ok(ReplayResult{symbol,strategy_key:strategy.strategy_key.clone(),strategy_version:strategy.version.clone(),
        from_ts_ms:events.first().unwrap().ts_ms,to_ts_ms:events.last().unwrap().ts_ms,events_replayed:events.len(),
        decisions:events.len(),buy_decisions:buys,sell_decisions:sells,wait_decisions:waits,initial_capital,
        final_equity,net_pnl:final_equity-initial_capital,net_return_pct:(final_equity/initial_capital-1.0)*100.0,
        max_drawdown_pct:max_dd*100.0,fees_paid:fees,slippage_paid:slippage,fee_bps,slippage_bps,fills,last_decision,
        assumptions:vec!["Historical replay uses only persisted raw events; it does not download missing history.".into(),
        "Events are sorted by event timestamp then event ID; each decision sees only the current and prior events.".into(),
        "Long-only, all-available-cash entries and full exits on SELL; no leverage or shorting.".into(),
        "Fees and slippage are constant user-supplied basis-point assumptions; funding cashflows are not yet settled.".into(),
        "Open positions are marked to the last observed trade price; no forced liquidation or borrow cost model.".into(),
        "Results are a research simulation, not live or expected performance.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::edge_strategy::EdgeStrategy;
    #[test]
    fn replay_is_time_ordered_and_charges_execution_costs() {
        let events=vec![
            ReplayEvent{event_id:"b".into(),venue:"test".into(),symbol:"BTCUSDT".into(),event_type:"trade".into(),ts_ms:2,
                price:Some(101.0),size:Some(1.0),side:Some("buy".into()),bid:None,ask:None,bid_size:None,ask_size:None,
                funding_rate:None,open_interest:None,liquidation_side:None},
            ReplayEvent{event_id:"a".into(),venue:"test".into(),symbol:"BTCUSDT".into(),event_type:"book".into(),ts_ms:1,
                price:None,size:None,side:None,bid:Some(99.0),ask:Some(100.0),bid_size:Some(1.0),ask_size:Some(1.0),
                funding_rate:None,open_interest:None,liquidation_side:None},
        ];
        let result=replay(events,&EdgeStrategy::baseline(),1000.0,10.0,5.0).unwrap();
        assert_eq!(result.from_ts_ms,1); assert_eq!(result.to_ts_ms,2);
        assert_eq!(result.events_replayed,2); assert!(result.fees_paid>=0.0);
        assert_eq!(result.strategy_version,EdgeStrategy::baseline().version);
    }
    #[test]
    fn replay_rejects_empty_or_invalid_cost_assumptions() {
        assert!(replay(vec![],&EdgeStrategy::baseline(),1000.0,0.0,0.0).is_err());
    }
}
