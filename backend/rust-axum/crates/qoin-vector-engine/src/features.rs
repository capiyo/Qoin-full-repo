//! Pure, causal market-feature calculations. Missing source data stays unavailable.
use serde::Serialize;
#[derive(Debug,Clone,Copy,Serialize,PartialEq,Eq)]#[serde(rename_all="snake_case")]pub enum MarketKind{Crypto,Forex,Equity,Commodity,FixedIncome,Other}
#[derive(Debug,Clone,Serialize,PartialEq)]pub struct Candle{pub timestamp_ms:i64,pub open:f64,pub high:f64,pub low:f64,pub close:f64,pub volume:f64}
#[derive(Debug,Clone,Serialize,PartialEq)]pub struct FeaturePoint{pub name:String,pub value:f64,pub confidence:f64,pub risk:f64,pub context:f64,pub available:bool}
impl FeaturePoint{pub fn unavailable(n:&str)->Self{Self{name:n.into(),value:0.0,confidence:0.0,risk:0.0,context:0.0,available:false}}pub fn new(n:&str,v:f64,c:f64)->Self{if !v.is_finite()||!c.is_finite(){return Self::unavailable(n)}Self{name:n.into(),value:v.clamp(-1.0,1.0),confidence:c.clamp(0.0,1.0),risk:0.0,context:0.0,available:true}}pub fn with_risk(n:&str,v:f64,c:f64,r:f64)->Self{let mut p=Self::new(n,v,c);if !r.is_finite(){return Self::unavailable(n)}p.risk=r.clamp(0.0,1.0);p}pub fn with_context(n:&str,v:f64,c:f64,r:f64,x:f64)->Self{let mut p=Self::with_risk(n,v,c,r);if !x.is_finite(){return Self::unavailable(n)}p.context=x.clamp(-1.0,1.0);p}}
#[derive(Debug,Clone,Copy,Serialize,PartialEq,Eq)]pub enum SwingDirection{Up,Down}
#[derive(Debug,Clone,Copy,Serialize,PartialEq)]pub struct ConfirmedSwing{pub low:f64,pub high:f64,pub direction:SwingDirection,pub confirmed_at_ms:i64}
#[derive(Debug,Clone,Copy,Serialize,PartialEq)]pub struct VolumeAtPrice{pub price:f64,pub volume:f64}
#[derive(Debug,Clone,Copy,Serialize,PartialEq)]pub struct ValueArea{pub poc:f64,pub vah:f64,pub val:f64,pub included_volume_fraction:f64}
#[derive(Debug,Clone,Copy,Serialize,PartialEq)]pub struct OptionExposure{pub gamma:f64,pub open_interest:f64,pub contract_multiplier:f64,pub dealer_gamma_sign:f64}
fn valid_candles(cs:&[Candle])->bool{!cs.is_empty()&&cs.iter().all(|c|c.timestamp_ms>0&&[c.open,c.high,c.low,c.close,c.volume].iter().all(|v|v.is_finite())&&c.low>=0.0&&c.high>=c.low&&c.open>=c.low&&c.open<=c.high&&c.close>=c.low&&c.close<=c.high&&c.volume>=0.0)&&cs.windows(2).all(|w|w[0].timestamp_ms<w[1].timestamp_ms)}
pub fn calculate_ofi(buy:f64,sell:f64)->FeaturePoint{if !buy.is_finite()||!sell.is_finite()||buy<0.0||sell<0.0||buy+sell<=f64::EPSILON{return FeaturePoint::unavailable("ofi")}FeaturePoint::new("ofi",(buy-sell)/(buy+sell),1.0)}
pub fn calculate_open_interest_pressure(previous:f64,current:f64)->FeaturePoint{calculate_open_interest_pressure_scaled(previous,current,0.02)}
pub fn calculate_open_interest_pressure_scaled(previous:f64,current:f64,neutral_change:f64)->FeaturePoint{if !previous.is_finite()||!current.is_finite()||previous<=0.0||current<0.0||!neutral_change.is_finite()||neutral_change<=0.0{return FeaturePoint::unavailable("open_interest_change")}let d=(current-previous)/previous;FeaturePoint::with_context("open_interest_change",0.0,1.0,0.0,d/(d.abs()+neutral_change))}
pub fn calculate_funding_pressure(rate:Option<f64>,neutral_band:f64)->FeaturePoint{let r=match rate{Some(v)if v.is_finite()=>v,_=>return FeaturePoint::unavailable("funding_pressure")};if !neutral_band.is_finite()||neutral_band<=0.0{return FeaturePoint::unavailable("funding_pressure")}FeaturePoint::new("funding_pressure",-(r/neutral_band).tanh(),0.9)}
pub fn calculate_liquidity_stress(spread:Option<f64>,reference:f64)->FeaturePoint{let s=match spread{Some(v)if v.is_finite()&&v>=0.0=>v,_=>return FeaturePoint::unavailable("liquidity_stress")};if !reference.is_finite()||reference<=0.0{return FeaturePoint::unavailable("liquidity_stress")}FeaturePoint::with_risk("liquidity_stress",0.0,0.95,((s/reference)-1.0).max(0.0).tanh())}
pub fn calculate_volatility(cs:&[Candle],window:usize)->FeaturePoint{calculate_volatility_scaled(cs,window,0.01)}
pub fn calculate_volatility_scaled(cs:&[Candle],window:usize,reference_volatility:f64)->FeaturePoint{if window<2||cs.len()<window+1||!valid_candles(cs)||!reference_volatility.is_finite()||reference_volatility<=0.0{return FeaturePoint::unavailable("volatility")}let x=&cs[cs.len()-window-1..];let r:Vec<f64>=x.windows(2).map(|w|(w[1].close/w[0].close).ln()).collect();if r.iter().any(|v|!v.is_finite()){return FeaturePoint::unavailable("volatility")}let m=r.iter().sum::<f64>()/r.len()as f64;let sd=(r.iter().map(|v|(v-m).powi(2)).sum::<f64>()/r.len()as f64).sqrt();FeaturePoint::with_risk("volatility",0.0,0.95,((sd/reference_volatility)-1.0).max(0.0).tanh())}
/// Uses only a swing whose pivot confirmation timestamp has passed.
pub fn calculate_fibonacci(price:f64,swing:Option<ConfirmedSwing>,tolerance:f64)->FeaturePoint{let s=match swing{Some(v)=>v,None=>return FeaturePoint::unavailable("fibonacci")};if !price.is_finite()||price<=0.0||!s.low.is_finite()||!s.high.is_finite()||s.low<0.0||s.high<=s.low||s.confirmed_at_ms<=0||!tolerance.is_finite()||tolerance<=0.0{return FeaturePoint::unavailable("fibonacci")}let r=match s.direction{SwingDirection::Up=>(s.high-price)/(s.high-s.low),SwingDirection::Down=>(price-s.low)/(s.high-s.low)};let d=[0.236_f64,0.382,0.5,0.618,0.786].iter().map(|l|(r-l).abs()).fold(f64::INFINITY,f64::min);FeaturePoint::new("fibonacci",(if s.direction==SwingDirection::Up{1.0}else{-1.0})*(1.0-d/tolerance).clamp(0.0,1.0),0.9)}
/// Three-candle FVG; later candles that fully cross a zone invalidate it.
pub fn calculate_fvg(cs:&[Candle],price:f64)->FeaturePoint{calculate_fvg_scaled(cs,price,0.002)}
pub fn calculate_fvg_scaled(cs:&[Candle],price:f64,distance_fraction:f64)->FeaturePoint{if cs.len()<3||!valid_candles(cs)||!price.is_finite()||price<=0.0||!distance_fraction.is_finite()||distance_fraction<=0.0{return FeaturePoint::unavailable("fvg")}let mut best:Option<f64>=None;for i in 2..cs.len(){let a=&cs[i-2];let c=&cs[i];let(lo,hi,sign)=if c.low>a.high{(a.high,c.low,1.0)}else if c.high<a.low{(c.high,a.low,-1.0)}else{continue};let later=&cs[i+1..];let invalid=later.iter().any(|b|if sign>0.0{b.low<=lo}else{b.high>=hi});if invalid{continue}let mitigation=if sign>0.0{later.iter().map(|b|((hi-b.low)/(hi-lo)).clamp(0.0,1.0)).fold(0.0,f64::max)}else{later.iter().map(|b|((b.high-lo)/(hi-lo)).clamp(0.0,1.0)).fold(0.0,f64::max)};let dist=if price<lo{lo-price}else if price>hi{price-hi}else{0.0};let strength=(1.0-dist/(hi-lo).max(price*distance_fraction)).clamp(0.0,1.0)*(1.0-mitigation);if strength>0.0{best=Some(sign*strength)}}match best{Some(v)=>FeaturePoint::new("fvg",v,0.85),None=>FeaturePoint::unavailable("fvg")}}
/// Volume-at-price value area expands from POC toward the larger adjacent bin.
pub fn calculate_value_area(bins:&[VolumeAtPrice],target:f64)->Option<ValueArea>{if bins.is_empty()||!target.is_finite()||!(0.5..=0.95).contains(&target)||bins.iter().any(|b|!b.price.is_finite()||!b.volume.is_finite()||b.price<=0.0||b.volume<0.0){return None}let mut s=bins.to_vec();s.sort_by(|a,b|a.price.total_cmp(&b.price));if s.windows(2).any(|w|(w[1].price-w[0].price).abs()<=f64::EPSILON){return None}let total=s.iter().map(|b|b.volume).sum::<f64>();if !total.is_finite()||total<=0.0{return None}let p=s.iter().enumerate().max_by(|a,b|a.1.volume.total_cmp(&b.1.volume))?.0;let(mut l,mut r)=(p,p);let mut inc=s[p].volume;while inc/total<target&&(l>0||r+1<s.len()){let lv=if l>0{s[l-1].volume}else{-1.0};let rv=if r+1<s.len(){s[r+1].volume}else{-1.0};if rv>lv{r+=1;inc+=s[r].volume}else if l>0{l-=1;inc+=s[l].volume}else{break}}Some(ValueArea{poc:s[p].price,vah:s[r].price,val:s[l].price,included_volume_fraction:inc/total})}
pub fn calculate_value_area_position(price:f64,area:Option<ValueArea>)->FeaturePoint{let a=match area{Some(v)=>v,None=>return FeaturePoint::unavailable("value_area")};if !price.is_finite()||price<=0.0||a.val<=0.0||a.vah<a.val||a.poc<a.val||a.poc>a.vah{return FeaturePoint::unavailable("value_area")}let width=(a.vah-a.val).max(a.poc*0.001);let v=if price>a.vah{((price-a.vah)/width).tanh()}else if price<a.val{-((a.val-price)/width).tanh()}else{((price-a.poc)/width).clamp(-1.0,1.0)*0.35};FeaturePoint::new("value_area",v,0.9)}
/// Dealer-signed GEX = Σ(gamma × OI × multiplier × spot² × 1% × dealer sign).
/// Requires a real options chain and documented dealer-side sign assumptions.
pub fn calculate_gex(spot:f64,options:&[OptionExposure],normalization_usd:f64)->FeaturePoint{if !spot.is_finite()||spot<=0.0||!normalization_usd.is_finite()||normalization_usd<=0.0||options.is_empty()||options.iter().any(|o|!o.gamma.is_finite()||o.gamma<0.0||!o.open_interest.is_finite()||o.open_interest<0.0||!o.contract_multiplier.is_finite()||o.contract_multiplier<=0.0||!o.dealer_gamma_sign.is_finite()||o.dealer_gamma_sign.abs()!=1.0){return FeaturePoint::unavailable("gex")}let x=options.iter().map(|o|o.gamma*o.open_interest*o.contract_multiplier*spot*spot*0.01*o.dealer_gamma_sign).sum::<f64>();if !x.is_finite(){return FeaturePoint::unavailable("gex")}FeaturePoint::with_context("gex",0.0,0.8,if x<0.0{(x.abs()/normalization_usd).tanh()}else{0.0},if x<0.0{-1.0}else if x>0.0{1.0}else{0.0})}
pub fn calculate_liquidation_pressure(long_volume:f64,short_volume:f64)->FeaturePoint{if !long_volume.is_finite()||!short_volume.is_finite()||long_volume<0.0||short_volume<0.0||long_volume+short_volume<=f64::EPSILON{return FeaturePoint::unavailable("liquidation_pressure")}FeaturePoint::new("liquidation_pressure",(short_volume-long_volume)/(long_volume+short_volume),0.85)}
pub fn calculate_event_intensity(event_count:f64,expected_rate:f64)->FeaturePoint{if !event_count.is_finite()||!expected_rate.is_finite()||event_count<0.0||expected_rate<=0.0{return FeaturePoint::unavailable("event_intensity")}FeaturePoint::with_risk("event_intensity",0.0,0.9,((event_count/expected_rate)-1.0).max(0.0).ln_1p().tanh())}
pub fn calculate_price_momentum(cs:&[Candle],window:usize,neutral_log_return:f64)->FeaturePoint{if window==0||cs.len()<window+1||!valid_candles(cs)||!neutral_log_return.is_finite()||neutral_log_return<=0.0{return FeaturePoint::unavailable("price_momentum")}let x=&cs[cs.len()-window-1..];let first=x[0].close;let last=x[x.len()-1].close;if first<=0.0||last<=0.0{return FeaturePoint::unavailable("price_momentum")}FeaturePoint::new("price_momentum",((last/first).ln()/neutral_log_return).tanh(),0.9)}
pub fn calculate_basis(spot:f64,derivative:f64,reference_bps:f64)->FeaturePoint{if !spot.is_finite()||!derivative.is_finite()||!reference_bps.is_finite()||spot<=0.0||derivative<=0.0||reference_bps<=0.0{return FeaturePoint::unavailable("basis")}FeaturePoint::with_context("basis",0.0,0.9,0.0,(((derivative/spot-1.0)*10000.0)/reference_bps).tanh())}
#[cfg(test)]mod tests{use super::*;fn bar(t:i64)->Candle{Candle{timestamp_ms:t,open:10.0,high:11.0,low:9.0,close:10.0,volume:1.0}}
#[test]fn ofi_signed_and_missing_safe(){assert!((calculate_ofi(80.0,20.0).value-0.6).abs()<1e-9);assert!(!calculate_ofi(0.0,0.0).available)}
#[test]fn fib_uses_confirmed_swing(){let s=ConfirmedSwing{low:100.0,high:200.0,direction:SwingDirection::Up,confirmed_at_ms:10};assert!(calculate_fibonacci(138.2,Some(s),0.03).value>0.9);assert!(!calculate_fibonacci(138.2,None,0.03).available)}
#[test]fn value_area_calculates_poc(){let a=calculate_value_area(&[VolumeAtPrice{price:100.0,volume:10.0},VolumeAtPrice{price:101.0,volume:50.0},VolumeAtPrice{price:102.0,volume:30.0},VolumeAtPrice{price:103.0,volume:10.0}],0.7).unwrap();assert_eq!(a.poc,101.0);assert!(a.vah>=102.0)}
#[test]fn gex_requires_options_chain(){assert!(!calculate_gex(100.0,&[],1e6).available);let o=[OptionExposure{gamma:0.02,open_interest:100.0,contract_multiplier:100.0,dealer_gamma_sign:1.0}];assert!(calculate_gex(100.0,&o,1e6).available)}
#[test]fn rejects_out_of_order_candles(){assert!(!calculate_volatility(&[bar(2),bar(1),bar(3)],2).available)}
}

/// Inputs for one timestamped feature calculation. Slices are supplied by the
/// venue adapter; every field is instrument/timeframe specific and past-only.
pub struct FeatureInputs<'a> {
    pub market: MarketKind,
    pub instrument: &'a str,
    pub timestamp_ms: i64,
    pub buy_volume: Option<f64>,
    pub liquidation_volumes: Option<(f64, f64)>,
    pub event_count: Option<f64>,
    pub expected_event_rate: f64,
    pub sell_volume: Option<f64>,
    pub open_interest: Option<(f64, f64)>,
    pub open_interest_neutral_change: f64,
    pub funding_rate: Option<f64>,
    pub funding_neutral_band: f64,
    pub spread_bps: Option<f64>,
    pub reference_spread_bps: f64,
    pub candles: &'a [Candle],
    pub volatility_window: usize,
    pub momentum_neutral_log_return: f64,
    pub reference_volatility: f64,
    pub fvg_distance_fraction: f64,
    pub current_price: f64,
    pub confirmed_swing: Option<ConfirmedSwing>,
    pub fibonacci_tolerance: f64,
    pub volume_profile: &'a [VolumeAtPrice],
    pub value_area_fraction: f64,
    pub options_chain: &'a [OptionExposure],
    pub gex_spot: Option<f64>,
    pub gex_normalization_usd: f64,
    pub spot_price: Option<f64>,
    pub derivative_price: Option<f64>,
    pub basis_reference_bps: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MarketFeatureVector {
    pub market: MarketKind,
    pub instrument: String,
    pub timestamp_ms: i64,
    pub points: Vec<FeaturePoint>,
}

/// Calculate every configured vector point independently from the same
/// timestamped market snapshot. Unsupported/missing market data stays unavailable.
pub fn calculate_feature_vector(input: &FeatureInputs<'_>) -> MarketFeatureVector {
    if input.timestamp_ms <= 0 || input.instrument.trim().is_empty()
        || input.candles.iter().any(|c| c.timestamp_ms > input.timestamp_ms)
        || input.confirmed_swing.map(|s| s.confirmed_at_ms > input.timestamp_ms).unwrap_or(false) {
        let names = ["ofi","open_interest_change","liquidation_pressure","event_intensity","price_momentum","funding_pressure","liquidity_stress","volatility","fibonacci","fvg","value_area","gex","basis"];
        return MarketFeatureVector { market: input.market, instrument: input.instrument.to_owned(),
            timestamp_ms: input.timestamp_ms, points: names.iter().map(|n| FeaturePoint::unavailable(n)).collect() };
    }
    let ofi = match (input.buy_volume, input.sell_volume) {
        (Some(buy), Some(sell)) => calculate_ofi(buy, sell),
        _ => FeaturePoint::unavailable("ofi"),
    };
    let oi = match input.open_interest {
        Some((previous, current)) => calculate_open_interest_pressure_scaled(previous, current, input.open_interest_neutral_change),
        None => FeaturePoint::unavailable("open_interest_change"),
    };
    let gex = match input.gex_spot {
        Some(spot) => calculate_gex(spot, input.options_chain, input.gex_normalization_usd),
        None => FeaturePoint::unavailable("gex"),
    };
    let liquidation = match input.liquidation_volumes { Some((long, short)) => calculate_liquidation_pressure(long, short), None => FeaturePoint::unavailable("liquidation_pressure") };
    let event_intensity = match input.event_count { Some(count) => calculate_event_intensity(count, input.expected_event_rate), None => FeaturePoint::unavailable("event_intensity") };
    let basis = match (input.spot_price, input.derivative_price) {
        (Some(spot), Some(derivative)) => calculate_basis(spot, derivative, input.basis_reference_bps),
        _ => FeaturePoint::unavailable("basis"),
    };
    let area = calculate_value_area(input.volume_profile, input.value_area_fraction);
    let points = vec![
        ofi,
        oi,
        liquidation,
        event_intensity,
        calculate_price_momentum(input.candles, input.volatility_window, input.momentum_neutral_log_return),
        calculate_funding_pressure(input.funding_rate, input.funding_neutral_band),
        calculate_liquidity_stress(input.spread_bps, input.reference_spread_bps),
        calculate_volatility_scaled(input.candles, input.volatility_window, input.reference_volatility),
        calculate_fibonacci(input.current_price, input.confirmed_swing, input.fibonacci_tolerance),
        calculate_fvg_scaled(input.candles, input.current_price, input.fvg_distance_fraction),
        calculate_value_area_position(input.current_price, area),
        gex,
        basis,
    ];
    MarketFeatureVector {
        market: input.market,
        instrument: input.instrument.to_owned(),
        timestamp_ms: input.timestamp_ms,
        points,
    }
}


#[cfg(test)]
mod vector_builder_tests {
    use super::*;
    #[test]
    fn builder_calculates_ten_market_points_and_marks_missing_sources() {
        let candles = vec![
            Candle{timestamp_ms:1,open:100.0,high:102.0,low:99.0,close:101.0,volume:10.0},
            Candle{timestamp_ms:2,open:101.0,high:103.0,low:100.0,close:102.0,volume:12.0},
            Candle{timestamp_ms:3,open:104.0,high:106.0,low:104.0,close:105.0,volume:15.0},
        ];
        let profile = vec![
            VolumeAtPrice{price:100.0,volume:10.0},
            VolumeAtPrice{price:101.0,volume:40.0},
            VolumeAtPrice{price:102.0,volume:30.0},
        ];
        let input=FeatureInputs{
            market:MarketKind::Crypto,instrument:"BTC-USD",timestamp_ms:3,
            buy_volume:Some(80.0),liquidation_volumes:Some((30.0,70.0)),event_count:Some(10.0),expected_event_rate:5.0,sell_volume:Some(20.0),open_interest:Some((100.0,110.0)),open_interest_neutral_change:0.02,
            funding_rate:Some(0.0001),funding_neutral_band:0.001,spread_bps:Some(2.0),
            reference_spread_bps:5.0,candles:&candles,volatility_window:2,reference_volatility:0.01,momentum_neutral_log_return:0.01,fvg_distance_fraction:0.002,current_price:105.0,
            confirmed_swing:Some(ConfirmedSwing{low:90.0,high:110.0,direction:SwingDirection::Up,confirmed_at_ms:3}),
            fibonacci_tolerance:0.03,volume_profile:&profile,value_area_fraction:0.7,
            options_chain:&[],gex_spot:None,gex_normalization_usd:1_000_000.0,
            spot_price:Some(105.0),derivative_price:Some(105.1),basis_reference_bps:10.0,
        };
        let vector=calculate_feature_vector(&input);
        assert_eq!(vector.points.len(),13);
        assert!(vector.points.iter().any(|p|p.name=="ofi"&&p.available));
        assert!(vector.points.iter().any(|p|p.name=="gex"&&!p.available));
        assert!(vector.points.iter().any(|p|p.name=="value_area"&&p.available));
    }
}
