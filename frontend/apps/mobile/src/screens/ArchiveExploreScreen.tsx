import React, { useEffect, useMemo, useRef, useState } from "react";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { Alert, Animated, Image } from "react-native";
import * as ImagePicker from "expo-image-picker";
import type { User } from "firebase/auth";
import { Linking, Modal, Pressable, ScrollView, StyleSheet, Text as NativeText, type TextProps, TextInput as NativeTextInput, type TextInputProps, View } from "react-native";
import Svg, { Circle, Line, Path, Polygon, Polyline, Rect, Text as SvgText } from "react-native-svg";
import { SafeAreaView } from "react-native-safe-area-context";
import { subscribeQoinAuth, signOutQoinUser, uploadQoinAvatar, getQoinArchivedSymbols, saveQoinArchivedSymbols, getQoinBrokerSelections, saveQoinBrokerSelections, getQoinBrokerReadiness, getInstrumentVector, getQoinMarketState, getInstrumentVectorHistory, getQoinEvents, getQoinSetLedger, getQoinCalendar, getQoinInstruments, getQoinNews, getQoinStrategy, getQoinAlgorithmCatalogue, getQoinBrokerCatalogue, runQoinBacktest, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinAlgorithmCatalogue, type QoinBacktestResult, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent, type QoinSetLedger, type QoinStrategy } from "@qoin/core";

type Market = QoinInstrument["asset_class"];
type Instrument = {
  symbol: string;
  engineSymbol: string;
  name: string;
  market: Market;
  detail: string;
  venue: string;
  engineVenue: string;
  provider: string;
  providerStatus?: string;
  archived: boolean;
  tint: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", engineSymbol: "BTCUSDT", name: "Bitcoin", market: "Crypto", detail: "Bitcoin perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#E6A23C" },
  { symbol: "ETH/USD", engineSymbol: "ETHUSDT", name: "Ethereum", market: "Crypto", detail: "Ether perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#6477DF" },
  { symbol: "SOL/USD", engineSymbol: "SOLUSDT", name: "Solana", market: "Crypto", detail: "Solana perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#8B72E8" },
  { symbol: "NVDA", engineSymbol: "NVDA", name: "NVIDIA", market: "Equities", detail: "Semiconductors · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#76A85A" },
  { symbol: "EUR/USD", engineSymbol: "OANDA:EUR_USD", name: "Euro / US Dollar", market: "FX", detail: "Major FX pair", venue: "OANDA", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#4D91D8" },
  { symbol: "AAPL", engineSymbol: "AAPL", name: "Apple", market: "Equities", detail: "Consumer technology · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#8895A7" },
];

type MainTab = "Archive" | "Explore";
type Props = { mode: MainTab; onAuthRequested: (mode: "signin" | "register") => void; onHeaderVisibilityChange: (visible: boolean) => void; headerVisible: boolean; onExploreRequested: () => void };

function Text(props: TextProps) {
  const flattened = StyleSheet.flatten(props.style);
  const weight = flattened?.fontWeight;
  const fontFamily = flattened?.fontFamily ?? (
    weight === "700" || weight === "800" || weight === "bold"
      ? "DMSans_700Bold"
      : weight === "500" || weight === "600"
        ? "DMSans_500Medium"
        : "DMSans_400Regular"
  );
  return <NativeText {...props} style={[{ fontFamily }, props.style]} />;
}

function TextInput(props: TextInputProps) {
  return <NativeTextInput {...props} selectionColor={props.selectionColor ?? "#7DF162"} style={[{ fontFamily: "DMSans_400Regular" }, props.style]} />;
}

type QoinNotification = { id: string; symbol: string; title: string; body: string; kind: "signal" | "liquidation"; createdAt: number; read: boolean; details: string };
const QOIN_FEATURE_SHORT_NAMES = ["OFI", "LQTY", "VOL", "FUND", "OIΔ", "LIQ", "BASIS"] as const;

type PercentageWidth = `${number}%`;

function percentageWidth(value: number, total: number): PercentageWidth {
  if (!Number.isFinite(value) || !Number.isFinite(total) || total <= 0) return "0%";
  const percentage = Math.max(0, Math.min(100, (value / total) * 100));
  return `${percentage}%`;
}

function FeatureSpace({ decision }: { decision: QoinVectorDecision | null }) {
  const labels = ["OFI", "LIQ", "VOL", "FUND", "OI Δ", "LIQ", "BASIS"];
  const values = QOIN_FEATURE_NAMES.map((name) => Math.max(-1, Math.min(1, decision?.features?.[name] ?? 0)));
  const coordinates = values.map((value, index) => {
    const angle = -Math.PI / 2 + index * Math.PI * 2 / labels.length;
    const radius = 18 + value * 14;
    return (50 + Math.cos(angle) * radius) + "," + (50 + Math.sin(angle) * radius);
  }).join(" ");
  return <View style={styles.featureSpacePanel}>
    <Text style={styles.attentionTitle}>Seven-force feature space</Text>
    <Text style={styles.attentionCaption}>Normalized engine inputs · not a learned embedding</Text>
    <Svg viewBox="0 0 100 100" width="100%" height={190}>
      {[14,24,34].map((radius) => <Polygon key={radius} points={labels.map((_,i) => { const a=-Math.PI/2+i*Math.PI*2/labels.length; return (50+Math.cos(a)*radius)+","+(50+Math.sin(a)*radius); }).join(" ")} fill="none" stroke="#365247" strokeWidth=".5" />)}
      {labels.map((label,i) => { const a=-Math.PI/2+i*Math.PI*2/labels.length; return <React.Fragment key={label}><Line x1="50" y1="50" x2={50+Math.cos(a)*35} y2={50+Math.sin(a)*35} stroke="#365247" strokeWidth=".5" /><SvgText x={50+Math.cos(a)*43} y={50+Math.sin(a)*43} fill="#7F8DA1" fontSize="4.5" textAnchor="middle">{label}</SvgText></React.Fragment>; })}
      {decision ? <Polygon points={coordinates} fill="#7DF162" fillOpacity=".16" stroke="#7DF162" strokeWidth="1.2" /> : <SvgText x="50" y="52" fill="#94A3B8" fontSize="5" textAnchor="middle">NO FEATURE VECTOR</SvgText>}
    </Svg>
    <View style={styles.featureValues}>{QOIN_FEATURE_NAMES.map((name,index) => <View key={name} style={styles.featureValue}><Text style={styles.metricLabel}>{labels[index]}</Text><Text style={styles.metricValue}>{values[index].toFixed(3)}</Text></View>)}</View>
  </View>;
}


function DecisionTracePanel({ decision }: { decision: QoinVectorDecision }) {
  const gates = decision.diagnostics;
  const rows = decision.feature_trace || [];
  const gateRows: [string, boolean | undefined][] = [
    ["Spread supplied", gates?.spread_present], ["Spread within limit", gates?.spread_within_limit], ["Quote fresh", gates?.spread_fresh],
    ["Alignment threshold", gates?.alignment_passed], ["Magnitude threshold", gates?.magnitude_passed],
    ["Angular threshold", gates?.angle_passed],
  ];
  return <View style={styles.tracePanel}>
    <Text style={styles.attentionTitle}>Decision trace · nothing hidden</Text>
    <Text style={styles.attentionCaption}>Exact calculations and parameters for {decision.strategy_version || "this decision"}</Text>
    {decision.input_event ? <View style={styles.traceSource}>
      <Text style={styles.engineLabel}>Trigger event · {decision.input_event.event_type} · {decision.input_event.venue}</Text>
      <Text style={styles.traceValue}>{decision.input_event.event_id}</Text>
      <Text style={styles.attentionCaption}>{new Date(decision.input_event.event_ts_ms).toLocaleString()} · {decision.input_event.state_event_count} accumulated events</Text>
    </View> : null}
    <ScrollView horizontal showsHorizontalScrollIndicator>
      <View>
        <View style={styles.traceTableRow}>
          {["Feature","Raw","Transform","Scale","Normalized","Clamped","Bull term","Bear term","OI interaction"].map((label) => <Text key={label} style={[styles.traceCell,styles.traceHead]}>{label}</Text>)}
        </View>
        {rows.map((item) => <View key={item.name} style={styles.traceTableRow}>
          {[item.name.replaceAll("_"," "),item.raw_value.toPrecision(4),item.transform,String(item.scale),item.normalized_value.toFixed(4),item.clamped_value.toFixed(4),item.bullish_weighted_term.toFixed(4),item.bearish_weighted_term.toFixed(4),(item.interaction_contribution ?? 0).toFixed(4)].map((value,index) => <Text key={index} style={styles.traceCell}>{value}</Text>)}
        </View>)}
      </View>
    </ScrollView>
    <View style={styles.traceGateGrid}>
      {gateRows.map(([label,value]) => <View key={label} style={styles.traceGate}>
        <Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value == null ? "Not reported" : value ? "PASS" : "FAIL"}</Text>
      </View>)}
    </View>
    <Text style={styles.disclaimer}>Angle {decision.angle_degrees?.toFixed(4) ?? "—"}° · angle limit {gates?.angle_limit_radians?.toFixed(4) ?? "—"} rad · score limit {gates?.signal_threshold ?? "—"} · spread limit {gates?.spread_limit_bps ?? "—"} bps. {decision.reason || ""}</Text>
    <Text style={styles.traceSubhead}>Raw inputs (null means not supplied)</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify({ ...decision.raw_inputs, liquidation_side: decision.liquidation_side }, null, 2)}</Text>
    <Text style={styles.traceSubhead}>Exact strategy parameter snapshot</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify(decision.strategy_parameters || null, null, 2)}</Text>
    <Text style={styles.traceSubhead}>Complete serialized decision</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify(decision, null, 2)}</Text>
  </View>;
}


function MobileVectorSpaceMechanics({ decision, marketState, lastEvent }: { decision: QoinVectorDecision | null; marketState: QoinMarketState | null; lastEvent: QoinStreamUpdate["event"] | null }) {
  const names = QOIN_FEATURE_NAMES;
  const raw = names.map((name) => decision?.features?.[name] ?? 0);
  const attended = names.map((_, index) => decision?.attended_features?.[index] ?? 0);
  const directions = names.map((_, index) => -Math.PI / 2 + index * Math.PI * 2 / names.length);
  const endpoints = attended.map((value, index) => ({
    x: 50 + Math.cos(directions[index]) * Math.max(-1, Math.min(1, value / 3)) * 34,
    y: 50 - Math.sin(directions[index]) * Math.max(-1, Math.min(1, value / 3)) * 34,
  }));
  const attention = decision?.attention ?? [];
  const links = names.flatMap((_, from) => names.map((__, to) => ({ from, to, weight: attention[from]?.[to] ?? 0 })).filter((link) => link.from !== link.to && link.weight >= 0.18))
    .sort((a, b) => b.weight - a.weight).slice(0, 12);
  const colors = ["#64A7E8","#64C69B","#D9A65D","#B69AE8","#DB7E85","#72C6CF","#E2B86A"];
  const output = decision?.decision_vector ?? [0, 0];
  return <View style={styles.tracePanel}>
    <Text style={styles.attentionTitle}>Vector space · seven engine dimensions</Text>
    <Text style={styles.attentionCaption}>Fixed 2D projection of the actual R⁷ feature state · decision output is a separate space</Text>
    <View style={{padding:11,borderRadius:12,backgroundColor:"#13231C",gap:8}}>
      <Text style={{color:"#7DD6A5",fontSize:9,fontWeight:"800"}}>LIVE PROVIDER INPUT · {lastEvent?.venue || "WAITING"}</Text>
      <Text style={{color:"#C5D0C7",fontSize:11,fontWeight:"700"}}>{lastEvent?.event_type?.replaceAll("_"," ") || "No incoming event"} · {lastEvent?.ts_ms ? new Date(lastEvent.ts_ms).toLocaleTimeString() : "timestamp unavailable"}</Text>
      <View style={{flexDirection:"row",flexWrap:"wrap",gap:12}}>
        {[
          ["Price",marketState?.last_price?.toLocaleString(undefined,{maximumFractionDigits:8})],
          ["Mark price",marketState?.mark_price?.toLocaleString(undefined,{maximumFractionDigits:8})],
          ["Basis bps",marketState?.basis_bps?.toFixed(3)],
          ["Flow imbalance",marketState?.order_flow_imbalance?.toFixed(5)],
          ["Book imbalance",marketState?.liquidity_available ? marketState.liquidity_imbalance?.toFixed(5) : "—"],
          ["OI change",marketState?.open_interest_change?.toFixed(5)],
          ["Liquidation pressure",marketState?.liquidation_pressure?.toFixed(4)],
          ["Event intensity",marketState?.event_intensity?.toFixed(5)],
          ["Spread bps",marketState?.spread_bps?.toFixed(3)],
          ["Quote age ms",marketState?.last_book_ts_ms != null && marketState?.last_event_ts_ms != null ? String(Math.max(0,marketState.last_event_ts_ms-marketState.last_book_ts_ms)) : "—"],
          ["Open interest",marketState?.open_interest?.toLocaleString(undefined,{maximumFractionDigits:4})],
          ["Funding",marketState?.funding_rate?.toFixed(7)],
        ].map(([label,value])=><View key={label} style={{minWidth:"28%"}}>
          <Text style={{color:"#91A99B",fontSize:8}}>{label}</Text>
          <Text style={{color:"#EDF4EE",fontSize:11,fontWeight:"800"}}>{value ?? "—"}</Text>
        </View>)}
      </View>
    </View>

    <View style={styles.mapWrap}>
      <Svg viewBox="0 0 100 100" width="100%" height={260}>
        <Rect width="100" height="100" fill="#0E1612"/>
        {[12,22,32].map((radius)=><Circle key={radius} cx="50" cy="50" r={radius} fill="none" stroke="#365247" strokeWidth=".35" strokeDasharray="1.5 2"/>)}
        <Line x1="5" y1="50" x2="95" y2="50" stroke="#668477" strokeWidth=".5"/>
        <Line x1="50" y1="5" x2="50" y2="95" stroke="#668477" strokeWidth=".5"/>
        {links.map((link,index)=><Line key={index} x1={endpoints[link.from].x} y1={endpoints[link.from].y} x2={endpoints[link.to].x} y2={endpoints[link.to].y} stroke="#D6A85C" strokeWidth={0.35+link.weight*1.1} opacity={0.18+link.weight*.55}/>)}
        {endpoints.map((point,index)=><React.Fragment key={names[index]}>
          <Line x1="50" y1="50" x2={point.x} y2={point.y} stroke={colors[index]} strokeWidth="1.2"/>
          <Circle cx={point.x} cy={point.y} r="1.8" fill={colors[index]}/>
          <SvgText x={50+Math.cos(directions[index])*43} y={50-Math.sin(directions[index])*43} fill="#D6E4DC" fontSize="3.8" textAnchor="middle">{names[index].slice(0,8)}</SvgText>
        </React.Fragment>)}
        <Circle cx="50" cy="50" r="1.8" fill="#7DF162"/>
      </Svg>
    </View>
    {names.map((name,index)=><View key={name} style={styles.metric}>
      <Text style={styles.metricLabel}>{name.replaceAll("_"," ")}</Text>
      <Text style={styles.metricValue}>{raw[index].toFixed(4)} → {attended[index].toFixed(4)}</Text>
    </View>)}
    <Text style={styles.attentionTitle}>Top directed attention links</Text>
    {links.slice(0,6).map((link,index)=><View key={index} style={styles.metric}>
      <Text style={styles.metricLabel}>{names[link.from]} → {names[link.to]}</Text>
      <Text style={styles.metricValue}>{link.weight.toFixed(4)}</Text>
    </View>)}
    <View style={styles.vectorOutputBridge}>
      <View style={{flex:1}}>
        <Text style={styles.engineLabel}>Resultant decision vector</Text>
        <Text style={styles.engineValue}>({output[0].toFixed(5)}, {output[1].toFixed(5)})</Text>
      </View>
      <View style={{flex:1}}>
        <Text style={styles.engineLabel}>Engine output</Text>
        <Text style={styles.engineValue}>{decision?.action?.toUpperCase() || "WAIT"}</Text>
      </View>
    </View>
    <Text style={styles.disclaimer}>Attention links are the engine's deterministic weights, not causal proof. The resultant vector and action are read from the backend decision and are not recalculated by this screen.</Text>
  </View>;
}

function MobileSetLedgerPanel({ ledger, status }: { ledger: QoinSetLedger | null; status: string }) {
  const firstSet = ledger?.sets[0];
  const secondSet = ledger?.sets[1];
  const firstMembers = new Set(firstSet?.member_event_ids ?? []);
  const secondMembers = new Set(secondSet?.member_event_ids ?? []);
  const intersectionCount = [...firstMembers].filter((id) => secondMembers.has(id)).length;
  const unionCount = new Set([...firstMembers, ...secondMembers]).size;
  const overlapMatrix = (ledger?.sets ?? []).map((set) => (ledger?.sets ?? []).map((other) => {
    const otherMembers = new Set(other.member_event_ids);
    const count = set.member_event_ids.filter((id) => otherMembers.has(id)).length;
    const denominator = Math.min(set.measure.counting_measure, other.measure.counting_measure);
    return { count, intensity: denominator > 0 ? count / denominator : 0 };
  }));
  return <View style={styles.tracePanel}>
    <Text style={styles.attentionTitle}>Version 2 · Set &amp; relation ledger</Text>
    <Text style={styles.attentionCaption}>Derived from persisted events · read-only · separate from V1 decisions</Text>
    <Text accessibilityLiveRegion="polite" style={styles.attentionCaption}>{status}</Text>
    {!ledger ? <Text style={styles.disclaimer}>{status}</Text> : <>
      {firstSet && secondSet ? <View style={styles.setVenn}>
        <Svg viewBox="0 0 320 150" width="100%" height={150}>
          <Circle cx="119" cy="72" r="54" fill="#7DF162" fillOpacity=".24" stroke="#7DF162" strokeWidth="1.5"/>
          <Circle cx="201" cy="72" r="54" fill="#159B78" fillOpacity=".24" stroke="#159B78" strokeWidth="1.5"/>
          <SvgText x="88" y="17" fill="#7DF162" fontSize="9" fontWeight="700" textAnchor="middle">{firstSet.label.slice(0,20)}</SvgText>
          <SvgText x="232" y="17" fill="#159B78" fontSize="9" fontWeight="700" textAnchor="middle">{secondSet.label.slice(0,20)}</SvgText>
          <SvgText x="91" y="76" fill="#C5D0C7" fontSize="14" fontWeight="700" textAnchor="middle">{Math.max(0,firstSet.measure.counting_measure-intersectionCount)}</SvgText>
          <SvgText x="160" y="76" fill="#C5D0C7" fontSize="14" fontWeight="700" textAnchor="middle">{intersectionCount}</SvgText>
          <SvgText x="229" y="76" fill="#C5D0C7" fontSize="14" fontWeight="700" textAnchor="middle">{Math.max(0,secondSet.measure.counting_measure-intersectionCount)}</SvgText>
          <SvgText x="160" y="139" fill="#89968D" fontSize="8" textAnchor="middle">Exclusive · shared · exclusive</SvgText>
        </Svg>
        <View style={styles.setVennUnion}><Text style={styles.attentionCaption}>Union cardinality</Text><Text style={styles.setOverlapCount}>{unionCount}</Text></View>
        <Text style={styles.attentionCaption}>Observed event-ID membership overlap. Circle areas are illustrative, not scaled to set size.</Text>
      </View> : null}
      <View style={{flexDirection:"row",flexWrap:"wrap",gap:12}}>
        <Text style={styles.engineValue}>Universe {ledger.universe_count}</Text>
        {ledger.source_event_count != null ? <Text style={styles.engineValue}>Source events {ledger.source_event_count}</Text> : null}
        {ledger.durable_observation_count != null ? <Text style={styles.engineValue}>Stored observations {ledger.durable_observation_count}</Text> : null}
        {ledger.durable_relation_count != null ? <Text style={styles.engineValue}>Stored relations {ledger.durable_relation_count}</Text> : null}
        {ledger.durable_history_complete != null ? <Text style={[styles.engineValue,{color:ledger.durable_history_complete ? "#7DF162" : "#B7791F"}]}>History {ledger.durable_history_complete ? "complete" : "incomplete"}</Text> : null}
        {ledger.relation_coverage_complete != null ? <Text style={[styles.engineValue,{color:ledger.relation_coverage_complete ? "#7DF162" : "#B7791F"}]}>Relation coverage {ledger.relation_coverage_complete ? "complete" : "bounded"}</Text> : null}
        <Text style={styles.engineValue}>Sets {ledger.sets.length}</Text>
        <Text style={styles.engineValue}>Relations {ledger.relations.length}</Text>
        <Text style={styles.engineValue}>Skipped {ledger.skipped_invalid_events}</Text>
        <Text style={styles.engineValue}>Limit {ledger.requested_limit}</Text>
      </View>
      {ledger.universe_truncated ? <Text style={[styles.disclaimer,{color:"#D99528"}]}>Universe capped at {ledger.requested_limit} source documents. Measures below apply only to this returned sample.</Text> : null}
      <Text style={styles.engineLabel}>SET CARDINALITY MAP · COUNTING MEASURE</Text>
      <View style={{gap:8,padding:10,borderRadius:11,backgroundColor:"#161F1A"}}>
        {ledger.sets.map((item) => <View key={"measure-"+item.set_id} style={{flexDirection:"row",alignItems:"center",gap:8}}>
          <Text numberOfLines={1} style={{width:112,color:"#C5D0C7",fontSize:9}}>{item.label}</Text>
          <View style={{height:9,flex:1,borderRadius:8,backgroundColor:"#243B28",overflow:"hidden"}}>
            <View style={{height:9,width:percentageWidth(item.measure.counting_measure,ledger.universe_count),borderRadius:8,backgroundColor:"#7DF162"}} />
          </View>
          <Text style={{width:24,textAlign:"right",color:"#EDF4EE",fontSize:10,fontWeight:"800"}}>{item.measure.counting_measure}</Text>
        </View>)}
        <Text style={styles.attentionCaption}>Each bar is the exact number of observed event IDs in that set, relative to the returned universe.</Text>
      </View>
      <Text style={styles.engineLabel}>SET INTERSECTION MATRIX · |A ∩ B|</Text>
      <ScrollView horizontal showsHorizontalScrollIndicator={false}>
        <View style={{gap:3,padding:4}}>
          <View style={{flexDirection:"row",alignItems:"center",gap:3}}>
            <View style={{width:106}} />
            {ledger.sets.map((item) => <Text key={"matrix-head-"+item.set_id} style={{width:24,textAlign:"center",color:"#89968D",fontSize:7,fontWeight:"800"}}>{item.set_id.split("_").map((part)=>part[0]).join("").toUpperCase()}</Text>)}
          </View>
          {ledger.sets.map((item,rowIndex) => <View key={"matrix-row-"+item.set_id} style={{flexDirection:"row",alignItems:"center",gap:3}}>
            <Text numberOfLines={1} style={{width:106,color:"#C5D0C7",fontSize:8}}>{item.label}</Text>
            {ledger.sets.map((other,columnIndex) => {
              const cell=overlapMatrix[rowIndex][columnIndex];
              return <View key={other.set_id} style={{width:24,height:22,borderRadius:4,alignItems:"center",justifyContent:"center",backgroundColor:"rgba(125, 241, 98,"+(0.06+cell.intensity*0.42)+")"}}>
                <Text style={{color:"#EDF4EE",fontSize:8,fontWeight:"700"}}>{cell.count}</Text>
              </View>;
            })}
          </View>)}
        </View>
      </ScrollView>
      <Text style={styles.attentionCaption}>Cells show exact shared event-ID counts. Darker cells mean greater overlap relative to the smaller set.</Text>
      <Text style={styles.engineLabel}>SET MEMBERSHIP & MEASURE</Text>
      {ledger.sets.map((item) => <View key={item.set_id} style={{padding:9,borderRadius:9,backgroundColor:"#161F1A",gap:4}}>
        <View style={{flexDirection:"row",justifyContent:"space-between",gap:8}}>
          <Text style={styles.traceValue}>{item.label}</Text><Text style={[styles.traceValue,{color:"#7DF162"}]}>{item.measure.counting_measure}</Text>
        </View>
        <Text style={styles.attentionCaption}>Counting measure · trade-size {item.measure.trade_size_measure.toFixed(6)} {item.measure.trade_size_unit} · observed {item.measure.trade_size_observed_count} / missing {item.measure.trade_size_missing_count}</Text>
        <Text style={styles.attentionCaption} numberOfLines={2}>Members: {item.member_event_ids.slice(0,5).join(", ") || "∅"}{item.member_event_ids.length>5 ? " … +" + (item.member_event_ids.length-5) : ""}</Text>
      </View>)}
      <Text style={styles.engineLabel}>RELATION FLOW · 60 SECOND WINDOW</Text>
      {ledger.relations.map((item) => <View key={"flow-"+item.relation_id} style={{padding:9,borderRadius:9,backgroundColor:"#161F1A",gap:4}}>
        <View style={{flexDirection:"row",alignItems:"center",gap:6}}>
          <Text numberOfLines={2} style={{flex:1,color:"#7DF162",fontSize:8,fontWeight:"700"}}>{item.source_set.replaceAll("_"," ")}</Text>
          <Svg viewBox="0 0 100 24" width={92} height={24}>
            <Circle cx="7" cy="12" r="4" fill="#7DF162"/>
            <Line x1="12" y1="12" x2="87" y2="12" stroke="#159B78" strokeWidth={item.edge_count ? 1.8 : .7} strokeDasharray={item.edge_count ? undefined : "2 2"}/>
            <SvgText x="85" y="16" fill="#159B78" fontSize="10" fontWeight="700">›</SvgText>
            <Circle cx="93" cy="12" r="4" fill="#159B78"/>
            <SvgText x="50" y="8" fill="#52637A" fontSize="5" textAnchor="middle">{item.edge_count} edges</SvgText>
          </Svg>
          <Text numberOfLines={2} style={{flex:1,textAlign:"right",color:"#159B78",fontSize:8,fontWeight:"700"}}>{item.target_set.replaceAll("_"," ")}</Text>
        </View>
        <View style={{flexDirection:"row",justifyContent:"space-between",gap:8}}>
          <Text style={styles.traceValue}>{item.label}</Text><Text style={[styles.traceValue,{color:"#7DF162"}]}>{item.edge_count} edges</Text>
        </View>
        <Text style={styles.attentionCaption}>Mean lag {item.mean_lag_ms == null ? "—" : item.mean_lag_ms.toFixed(1) + " ms"} · {item.interpretation}</Text>
        {item.edges.slice(0,4).map((edge,index) => <Text key={item.relation_id+index} style={styles.attentionCaption}>{edge.from_event_id} → {edge.to_event_id} · {edge.lag_ms} ms</Text>)}
        {item.edge_count > 4 ? <Text style={styles.attentionCaption}>Showing 4 of {item.edge_count} relation edges{item.edges_truncated ? " (sample capped at 100)" : ""}</Text> : null}
      </View>)}
      {ledger.limitations.map((item) => <Text key={item} style={styles.disclaimer}>• {item}</Text>)}
      <Text style={styles.attentionCaption}>Schema {ledger.schema_version} · {ledger.persistence}</Text>
    </>}
  </View>;
}

function MobileBacktestPanel({ symbol, venue, authenticated, onRequireAuth }: { symbol: string; venue: string; authenticated: boolean; onRequireAuth: () => void }) {
  const [capital, setCapital] = useState("10000");
  const [fee, setFee] = useState("10");
  const [slippage, setSlippage] = useState("5");
  const [latency, setLatency] = useState("0");
  const [participation, setParticipation] = useState("0.1");
  const [impact, setImpact] = useState("5");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [result, setResult] = useState<QoinBacktestResult | null>(null);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const [pendingRun, setPendingRun] = useState(false);
  const run = async () => {
    if (!authenticated) { setPendingRun(true); onRequireAuth(); setStatus("Sign in to continue the historical research replay."); return; }
    const initialCapital = Number(capital), feeBps = Number(fee), slippageBps = Number(slippage);
    const latencyMs = Number(latency), participationRate = Number(participation), impactBps = Number(impact);
    const fromMs = from.trim() ? Date.parse(from) : undefined;
    const toMs = to.trim() ? Date.parse(to) : undefined;
    if ((from.trim() && !Number.isFinite(fromMs)) || (to.trim() && !Number.isFinite(toMs)) ||
        (fromMs !== undefined && toMs !== undefined && fromMs > toMs)) {
      setStatus("Enter a valid date range; the start must not be after the end.");
      return;
    }
    if (!Number.isFinite(initialCapital) || initialCapital <= 0 || !Number.isFinite(feeBps) || feeBps < 0 ||
        !Number.isFinite(slippageBps) || slippageBps < 0 || !Number.isInteger(latencyMs) || latencyMs < 0 || latencyMs > 60000 ||
        !Number.isFinite(participationRate) || participationRate <= 0 || participationRate > 1 ||
        !Number.isFinite(impactBps) || impactBps < 0 || impactBps > 1000 || slippageBps + impactBps >= 10000) {
      setStatus("Check capital, non-negative costs, latency (0–60000 ms), participation (0–1] and impact (0–1000 bps).");
      return;
    }
    setBusy(true); setStatus(""); setResult(null);
    try {
      const replay = await runQoinBacktest({
        symbol, venue, from_ts_ms: fromMs, to_ts_ms: toMs, initial_capital: initialCapital,
        fee_bps: feeBps, slippage_bps: slippageBps, latency_ms: latencyMs,
        max_participation_rate: participationRate, market_impact_bps: impactBps, limit: 10000,
      });
      setResult(replay); setStatus("Replay completed from persisted events only.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Historical replay unavailable.");
    } finally { setBusy(false); }
  };
  useEffect(() => {
    if (authenticated && pendingRun && !busy) { setPendingRun(false); void run(); }
  }, [authenticated, pendingRun, busy]);
  const fields: Array<[string, string, (value: string) => void, "decimal-pad" | "default"]> = [
    ["Starting capital", capital, setCapital, "decimal-pad"],
    ["Fee (bps)", fee, setFee, "decimal-pad"],
    ["Slippage (bps)", slippage, setSlippage, "decimal-pad"],
    ["Latency (ms)", latency, setLatency, "decimal-pad"],
    ["Max participation (0–1)", participation, setParticipation, "decimal-pad"],
    ["Market impact (bps)", impact, setImpact, "decimal-pad"],
    ["From (YYYY-MM-DDTHH:mm)", from, setFrom, "default"],
    ["To (YYYY-MM-DDTHH:mm)", to, setTo, "default"],
  ];
  return <View style={styles.mobileBacktest}>
    <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Historical event replay</Text><Text style={styles.attentionCaption}>Paper only · no orders</Text></View>
    <Text style={styles.disclaimer}>Uses stored events for {venue}. No missing history is downloaded. Empty dates use the stored range. The active Rust V1 strategy is replayed; no order is sent.</Text>
    <View style={styles.backtestInputs}>
      {fields.map(([label, value, setter, keyboardType]) => (
        <View key={label} style={styles.backtestInputWrap}>
          <Text style={styles.engineLabel}>{label}</Text>
          <TextInput accessibilityLabel={label} value={value} onChangeText={setter} keyboardType={keyboardType} autoCapitalize="none" style={styles.backtestInput} placeholder={label.startsWith("From") || label.startsWith("To") ? "Optional" : undefined} />
        </View>
      ))}
    </View>
    <Pressable onPress={run} disabled={busy} style={[styles.backtestRun, busy && styles.backtestDisabled]}><Text style={styles.backtestRunText}>{busy ? "Replaying…" : "Run historical replay"}</Text></Pressable>
    {status ? <Text accessibilityLiveRegion="polite" style={styles.disclaimer}>{status}</Text> : null}
    {result ? <>
      <View style={styles.backtestMetrics}>
        {[
          ["Net P/L", result.net_pnl.toFixed(4)], ["Return", result.net_return_pct.toFixed(3) + "%"],
          ["Max drawdown", result.max_drawdown_pct.toFixed(3) + "%"], ["Final equity", result.final_equity.toFixed(4)],
          ["Events", String(result.events_replayed)], ["Largest raw-event gap", (result.max_event_gap_ms / 1000).toFixed(1) + "s"],
          ["Gaps over 5 minutes", String(result.event_gaps_over_5m)], ["BUY / SELL / WAIT", result.buy_decisions + " / " + result.sell_decisions + " / " + result.wait_decisions],
          ["Fees", result.fees_paid.toFixed(4)], ["Slippage", result.slippage_paid.toFixed(4)],
          ["Net funding · received +", result.net_funding_cashflow.toFixed(4)], ["Funding settlements", String(result.funding_settlements)],
          ["Market impact", result.market_impact_paid.toFixed(4)], ["Latency", result.latency_ms + " ms"],
        ].map(([label, value]) => <View key={label} style={styles.backtestMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
      </View>
      <Text style={styles.disclaimer}>Strategy {result.strategy_key}@{result.strategy_version} · {result.venue} · {result.events_replayed} observations.</Text>
      {result.execution_stress_scenarios.map((scenario) => <View key={scenario.name} style={styles.backtestFill}>
        <Text style={styles.newsMeta}>{scenario.name} · {scenario.cost_multiplier.toFixed(1)}× execution costs</Text>
        <Text style={styles.engineValue}>Estimated net P/L {scenario.estimated_net_pnl.toFixed(4)} · Return {scenario.estimated_return_pct.toFixed(3)}%</Text>
        <Text style={styles.disclaimer}>Costs are repriced without recomputing fills or liquidity.</Text>
      </View>)}
      {result.unfilled_pending_order ? <Text style={styles.disclaimer}>Unfilled pending {result.unfilled_pending_order.action.toUpperCase()} order · {result.unfilled_pending_order.filled_quantity.toPrecision(6)} filled; {result.unfilled_pending_order.remaining_quantity?.toPrecision(6) ?? "unknown"} remaining.</Text> : null}
      {result.fills.map((fill, index) => <View key={fill.order_id + ":" + index} style={styles.backtestFill}>
        <Text style={styles.newsMeta}>{fill.action} · {new Date(fill.ts_ms).toLocaleString()}</Text>
        <Text style={styles.engineValue}>Price {fill.price.toPrecision(8)} · Qty {fill.quantity.toPrecision(6)}</Text>
        <Text style={styles.disclaimer}>Fee {fill.fee.toFixed(4)} · Slippage {fill.slippage_cost.toFixed(4)} · Impact {fill.market_impact_cost.toFixed(4)} · Realized {fill.realized_pnl.toFixed(4)}</Text>
      </View>)}
      <Text style={styles.traceJson}>{result.assumptions.join("\n")}</Text>
    </> : null}
  </View>;
}
type MobileLedgerRecord = { id: string; timestamp: number | null; kind: "decision" | "observation" | "history"; title: string; summary: string; reference: string };
function MobileResearchLedger({ decision, history, events }: { decision: QoinVectorDecision | null; history: QoinVectorHistory[]; events: QoinMarketEvent[] }) {
  const records: MobileLedgerRecord[] = [
    ...events.map((event) => ({
      id: "event:" + event.event_id,
      timestamp: event.ts_ms,
      kind: "observation" as const,
      title: event.event_type.replaceAll("_", " "),
      summary: [event.price != null ? "Price " + event.price : null, event.size != null ? "Size " + event.size : null, event.side ? "Side " + event.side : null, event.funding_rate != null ? "Funding " + event.funding_rate : null].filter(Boolean).join(" · ") || "Provider observation recorded",
      reference: event.event_id,
    })),
    ...history.map((item, index) => ({
      id: "history:" + (item.stored_at || index),
      timestamp: item.stored_at ? Date.parse(item.stored_at) : null,
      kind: "history" as const,
      title: "Historical engine decision · " + item.action.toUpperCase(),
      summary: "Magnitude " + item.magnitude.toFixed(4) + " · " + (item.strategy_version || "strategy version unavailable"),
      reference: item.stored_at || "Historical vector record",
    })),
    ...(decision ? [{
      id: "decision:" + (decision.input_event?.event_id || decision.strategy_version || decision.symbol),
      timestamp: decision.input_event?.event_ts_ms ?? null,
      kind: "decision" as const,
      title: "Latest engine evaluation · " + (decision.action || "wait").toUpperCase(),
      summary: decision.reason || "No rationale was supplied by the engine.",
      reference: decision.input_event?.event_id || "Source event reference unavailable",
    }] : []),
  ].sort((left, right) => (right.timestamp ?? -1) - (left.timestamp ?? -1)).slice(0, 40);
  return <View style={{ gap: 10, padding: 13, borderRadius: 16, borderWidth: 0, borderColor: "transparent", backgroundColor: "#161F1A" }}>
    <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8 }}>
      <View style={{ flex: 1 }}>
        <Text style={{ color: "#EDF4EE", fontSize: 14, fontWeight: "800" }}>Decision &amp; evidence ledger</Text>
        <Text style={{ marginTop: 4, color: "#89968D", fontSize: 10 }}>Readable history of engine evaluations and recorded market evidence.</Text>
      </View>
      <Text style={{ color: "#7DF162", fontSize: 10, fontWeight: "800" }}>{records.length} RECORDS</Text>
    </View>
    {records.length ? records.map((record) => <View key={record.id} style={{ padding: 11, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: record.kind === "decision" ? "#161F1A" : "#161F1A", gap: 6 }}>
      <View style={{ flexDirection: "row", alignItems: "center", justifyContent: "space-between", flexWrap: "wrap", gap: 6 }}>
        <Text style={{ color: record.kind === "decision" ? "#7DF162" : "#89968D", fontSize: 9, fontWeight: "800", textTransform: "uppercase" }}>{record.kind}</Text>
        <Text style={{ color: "#9AA6B5", fontSize: 9 }}>{record.timestamp != null && Number.isFinite(record.timestamp) ? new Date(record.timestamp).toLocaleString() : "Timestamp unavailable"}</Text>
      </View>
      <Text style={{ color: "#EDF4EE", fontSize: 11, fontWeight: "800", textTransform: "capitalize" }}>{record.title}</Text>
      <Text style={{ color: "#C5D0C7", fontSize: 10, lineHeight: 15 }}>{record.summary}</Text>
      <Text selectable style={{ color: "#89968D", fontSize: 9 }}>Source reference · {record.reference}</Text>
    </View>) : <View style={{ padding: 14, borderRadius: 11, backgroundColor: "#0E1612" }}><Text style={{ color: "#89968D", fontSize: 11 }}>No persisted evidence or decision records are available for this instrument yet.</Text></View>}
    <Text style={{ color: "#9AA6B5", fontSize: 9, lineHeight: 14 }}>Read-only research history. This view reflects records returned by the API; it does not claim immutable audit storage or persisted set relationships.</Text>
  </View>;
}


const MOBILE_BROKER_DIRECTORY = [
  {id:"binance_usdm",name:"Binance USD-M",kind:"Crypto exchange",status:"Qoin execution adapter pending",selectable:true},
  {id:"coinbase_advanced_trade",name:"Coinbase Advanced Trade",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"kraken_spot",name:"Kraken Spot",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"bybit",name:"Bybit",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"okx",name:"OKX",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"bitget",name:"Bitget",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"gate_io",name:"Gate.io",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"kucoin",name:"KuCoin",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"deribit",name:"Deribit",kind:"Crypto exchange",status:"API listed · Qoin adapter pending",selectable:true},
  {id:"hyperliquid",name:"Hyperliquid",kind:"Crypto exchange",status:"Wallet authorization and adapter pending",selectable:true},
  {id:"oanda",name:"OANDA",kind:"Forex broker",status:"v20 API listed · Qoin adapter pending",selectable:true},
  {id:"ig",name:"IG",kind:"Forex broker",status:"API listed · account eligibility and adapter pending",selectable:true},
  {id:"fxcm",name:"FXCM",kind:"Forex broker",status:"Trading API listed · Qoin adapter pending",selectable:true},
  {id:"capital_com",name:"Capital.com",kind:"Forex broker",status:"REST/WebSocket API listed · adapter pending",selectable:true},
  {id:"deriv",name:"Deriv",kind:"Forex broker",status:"WebSocket API listed · account authorization pending",selectable:true},
  {id:"xtb",name:"XTB",kind:"Forex broker",status:"xAPI access must be confirmed",selectable:true},
  {id:"pepperstone",name:"Pepperstone",kind:"Forex broker",status:"cTrader API route · Qoin adapter pending",selectable:true},
  {id:"ic_markets",name:"IC Markets",kind:"Forex broker",status:"cTrader API route · Qoin adapter pending",selectable:true},
  {id:"forex_com",name:"FOREX.com",kind:"Forex broker",status:"REST API · regional eligibility pending",selectable:true},
  {id:"cmc_markets",name:"CMC Markets",kind:"Forex broker",status:"API access and Qoin adapter pending",selectable:true},
] as const;

function MobileBrokerCollectionPanel() {
  const [directory,setDirectory]=useState<Array<{id:string;name:string;kind:string;status:string;selectable:boolean}>>([...MOBILE_BROKER_DIRECTORY]);
  const [selected,setSelected]=useState<string[]>([]);
  const [capital,setCapital]=useState("100");
  const [currency,setCurrency]=useState("USD");
  const [preferencesReady,setPreferencesReady]=useState(false);
  const [readiness,setReadiness]=useState<{selected_count:number;execution_ready_count:number;connected_account_count:number;can_submit:boolean;dispatch_enabled:boolean;message:string;blockers:string[]}|null>(null);
  useEffect(()=>{
    let active=true;
    void getQoinBrokerCatalogue().then((catalogue)=>{if(active)setDirectory(catalogue.items.map((broker)=>({id:broker.id,name:broker.name,kind:broker.provider_kind==="exchange"?"Crypto exchange":"Forex broker",status:"Qoin "+broker.execution_status.replaceAll("_"," "),selectable:broker.selectable})));}).catch(()=>undefined);
    void AsyncStorage.getItem("qoin:broker-routing-preferences").then((raw)=>{
      if(!active||!raw)return;
      try{const parsed:unknown=JSON.parse(raw);if(Array.isArray(parsed))setSelected(parsed.filter((id):id is string=>typeof id==="string"&&MOBILE_BROKER_DIRECTORY.some((broker)=>broker.id===id&&broker.selectable)).slice(0,20));}catch{/* Ignore invalid local preference data. */}
    }).catch(()=>undefined).then(async()=>{try{const remote=await getQoinBrokerSelections();if(active&&remote.length)setSelected(remote);}catch{/* Guest users retain device-local preferences. */}}).finally(()=>{if(active)setPreferencesReady(true);});
    return()=>{active=false;};
  },[]);
  useEffect(()=>{if(preferencesReady){void AsyncStorage.setItem("qoin:broker-routing-preferences",JSON.stringify(selected)).catch(()=>undefined);void saveQoinBrokerSelections(selected).catch(()=>undefined);}},[selected,preferencesReady]);
  useEffect(()=>{
    let active=true;
    void getQoinBrokerReadiness(selected).then((result)=>{if(active)setReadiness(result);}).catch(()=>{if(active)setReadiness(null);});
    return()=>{active=false;};
  },[selected]);
  const amount=Number(capital),valid=Number.isFinite(amount)&&amount>0;
  const perAccount=valid&&selected.length?amount/selected.length:null;
  return <View style={{gap:11,padding:14,borderWidth:0,borderColor:"transparent",borderRadius:15,backgroundColor:"#161F1A"}}>
    <View style={{flexDirection:"row",justifyContent:"space-between",gap:8,alignItems:"center"}}><View style={{flex:1,gap:4}}><Text style={styles.kicker}>EXECUTION VENUES</Text><Text style={styles.newsTitle}>Broker collection</Text><Text style={styles.noteCopy}>Choose providers for Qoin’s execution-readiness check. Account authorization is separate; trades remain blocked until every selected account is verified.</Text></View><Text style={styles.coverageStatus}>{selected.length}/20</Text></View>
    {directory.map((broker)=><View key={broker.id} style={{gap:6,paddingVertical:10,borderTopWidth:0,borderTopColor:"transparent"}}>
      <View style={{flexDirection:"row",alignItems:"center",justifyContent:"space-between",gap:8}}><View style={{flex:1,gap:3}}><Text style={{color:"#EDF4EE",fontSize:12,fontWeight:"800"}}>{broker.name}</Text><Text style={{color:"#89968D",fontSize:8}}>{broker.kind}</Text></View>
        {broker.selectable?<Pressable accessibilityRole="button" accessibilityState={{selected:selected.includes(broker.id)}} onPress={()=>setSelected((current)=>current.includes(broker.id)?current.filter((id)=>id!==broker.id):current.length<20?[...current,broker.id]:current)} style={{paddingVertical:7,paddingHorizontal:12,borderRadius:20,backgroundColor:selected.includes(broker.id)?"#243B28":"#17291F"}}><Text style={{color:selected.includes(broker.id)?"#7DF162":"#7DF162",fontSize:9,fontWeight:"800"}}>{selected.includes(broker.id)?"Selected":"Select"}</Text></Pressable>:<Text style={{color:"#A36B12",fontSize:8,fontWeight:"800"}}>DATA ONLY</Text>}
      </View>
      <Text style={{color:"#89968D",fontSize:9,lineHeight:14}}>{broker.status}</Text>
    </View>)}
    <View accessibilityLiveRegion="polite" style={{gap:5,padding:11,borderRadius:11,backgroundColor:"#302718"}}><Text style={styles.engineLabel}>TRADE CONNECTION STATUS</Text><Text style={styles.engineValue}>{readiness?`${readiness.connected_account_count} connected · ${readiness.execution_ready_count}/${readiness.selected_count} ready · ${readiness.dispatch_enabled?"Dispatch enabled":"Dispatch blocked"}`:"Checking selected providers…"}</Text><Text style={styles.coverageDisclaimer}>{readiness?.message||"Waiting for broker readiness API."}</Text>{readiness?.blockers?.map((item)=><Text key={item} style={styles.coverageDisclaimer}>{item}</Text>)}</View>
    <View style={{gap:7,padding:11,borderRadius:11,backgroundColor:"#161F1A"}}>
      <Text style={styles.engineLabel}>PORTFOLIO BASE CURRENCY</Text>
      <View style={{flexDirection:"row",flexWrap:"wrap",gap:6}}>{["USD","EUR","GBP","KES","USDT"].map((value)=><Pressable key={value} onPress={()=>setCurrency(value)} style={{paddingVertical:6,paddingHorizontal:10,borderRadius:18,backgroundColor:currency===value?"#243B28":"#161F1A",borderWidth:0,borderColor:currency===value?"#FFFFFF14":"#FFFFFF14"}}><Text style={{color:currency===value?"#7DF162":"#89968D",fontSize:8,fontWeight:"800"}}>{value}</Text></Pressable>)}</View>
      <Text style={styles.engineLabel}>EQUAL TARGET NOTIONAL · {currency} · PREVIEW</Text>
      <TextInput value={capital} onChangeText={setCapital} keyboardType="decimal-pad" accessibilityLabel={"Portfolio notional in "+currency+" for allocation preview"} style={[styles.backtestInput,{backgroundColor:"#161F1A"}]} placeholder="Portfolio notional"/>
      <View style={{flexDirection:"row",justifyContent:"space-between"}}><Text style={styles.attentionCaption}>Selected venue preferences</Text><Text style={styles.engineValue}>{selected.length}</Text></View>
      <View style={{flexDirection:"row",justifyContent:"space-between"}}><Text style={styles.attentionCaption}>Illustrative target per selected venue · {currency}</Text><Text style={styles.engineValue}>{perAccount==null?"—":perAccount.toLocaleString(undefined,{maximumFractionDigits:2})}</Text></View>
    </View>
    <Text style={styles.coverageDisclaimer}>{selected.length?"Signed-in preferences are saved to the user profile and checked by the backend readiness API. Account linking and server-side eligibility checks remain required.":"No execution account is connected. Select a listed adapter to save a local preference."} Equal split is a target notional, not a guaranteed fill. Backend must validate minimums, margin, risk, costs and every selected account before routing.</Text>
  </View>;
}

export function ArchiveExploreScreen({ mode, onAuthRequested, onHeaderVisibilityChange, headerVisible, onExploreRequested }: Props) {
  const [visitedModes, setVisitedModes] = useState<Record<MainTab, boolean>>({ Archive: true, Explore: false });
  const archiveScrollRef = useRef<ScrollView | null>(null);
  const exploreScrollRef = useRef<ScrollView | null>(null);
  const scrollOffsetsRef = useRef<Record<MainTab, number>>({ Archive: 0, Explore: 0 });
  const lastScrollYRef = useRef<Record<MainTab, number>>({ Archive: 0, Explore: 0 });
  const downScrollDistanceRef = useRef<Record<MainTab, number>>({ Archive: 0, Explore: 0 });
  const headerTranslateY = useRef(new Animated.Value(0)).current;
  useEffect(() => {
    Animated.timing(headerTranslateY, { toValue: headerVisible ? 0 : -112, duration: headerVisible ? 220 : 260, useNativeDriver: true }).start();
  }, [headerVisible, headerTranslateY]);
  useEffect(() => {
    setVisitedModes((current) => current[mode] ? current : { ...current, [mode]: true });
  }, [mode]);
  useEffect(() => {
    lastScrollYRef.current[mode] = scrollOffsetsRef.current[mode];
    downScrollDistanceRef.current[mode] = 0;
    onHeaderVisibilityChange(true);
    const activeScrollRef = mode === "Archive" ? archiveScrollRef : exploreScrollRef;
    const frame = requestAnimationFrame(() => {
      activeScrollRef.current?.scrollTo({ y: scrollOffsetsRef.current[mode], animated: false });
    });
    return () => cancelAnimationFrame(frame);
  }, [mode, onHeaderVisibilityChange, visitedModes[mode]]);
  const handleWorkspaceScroll = (currentMode: MainTab, event: { nativeEvent: { contentOffset: { y: number } } }) => {
    const nextY = Math.max(0, event.nativeEvent.contentOffset.y);
    const previousY = lastScrollYRef.current[currentMode];
    if (nextY <= 0) {
      downScrollDistanceRef.current[currentMode] = 0;
      onHeaderVisibilityChange(true);
    } else {
      const delta = nextY - previousY;
      if (delta > 0) {
        downScrollDistanceRef.current[currentMode] += delta;
        if (downScrollDistanceRef.current[currentMode] >= 6) onHeaderVisibilityChange(false);
      } else if (delta < 0) {
        downScrollDistanceRef.current[currentMode] = 0;
        onHeaderVisibilityChange(true);
      }
    }
    scrollOffsetsRef.current[currentMode] = nextY;
    lastScrollYRef.current[currentMode] = nextY;
  };
  const [items, setItems] = useState(seed);
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [mechanicsTab, setMechanicsTab] = useState<"vector" | "sets">("vector");
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [strategy, setStrategy] = useState<QoinStrategy | null>(null);
  const [catalogue, setCatalogue] = useState<QoinAlgorithmCatalogue | null>(null);
  const [catalogueStatus, setCatalogueStatus] = useState("loading");
  const [vectorLoading, setVectorLoading] = useState(false);
  const [vectorError, setVectorError] = useState("");
  const [vectorHistory, setVectorHistory] = useState<QoinVectorHistory[]>([]);
  const [rawEvents, setRawEvents] = useState<QoinMarketEvent[]>([]);
  const [setLedger, setSetLedger] = useState<QoinSetLedger | null>(null);
  const [setLedgerStatus, setSetLedgerStatus] = useState("Select an instrument to inspect V2 sets.");
  const [showRawEvents, setShowRawEvents] = useState(true);
  const [liveBySymbol, setLiveBySymbol] = useState<Record<string, { state?: QoinMarketState; decision?: QoinVectorDecision; event?: QoinStreamUpdate["event"] }>>({});
  const [marketState, setMarketState] = useState<QoinMarketState | null>(null);
  const [streamStatus, setStreamStatus] = useState("disconnected");
  const [lastEvent, setLastEvent] = useState<QoinStreamUpdate["event"] | null>(null);
  const [news, setNews] = useState<QoinNewsItem[]>([]);
  const [newsStatus, setNewsStatus] = useState("loading");
  const [newsCategory, setNewsCategory] = useState("general");
  const [macroEvents, setMacroEvents] = useState<QoinMacroEvent[]>([]);
  const [calendarStatus, setCalendarStatus] = useState("loading");
  const [notifications, setNotifications] = useState<QoinNotification[]>([]);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const [accountOpen, setAccountOpen] = useState(false);
  const [profileOpen, setProfileOpen] = useState(false);
  const [firebaseUser, setFirebaseUser] = useState<User | null>(null);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [avatarBusy, setAvatarBusy] = useState(false);
  const [algorithmBySymbol, setAlgorithmBySymbol] = useState<Record<string, 0 | 1>>({});
  const [cardLedgers, setCardLedgers] = useState<Record<string,QoinSetLedger>>({});
  const [cardLedgerStatus, setCardLedgerStatus] = useState<Record<string,"loading"|"unavailable">>({});
  const cardLedgerRequestsRef = useRef<Set<string>>(new Set());
  const loadCardLedger = (item: Instrument) => {
    if (cardLedgers[item.symbol] || cardLedgerRequestsRef.current.has(item.symbol)) return;
    cardLedgerRequestsRef.current.add(item.symbol);
    setCardLedgerStatus((current)=>({...current,[item.symbol]:"loading"}));
    void getQoinSetLedger(item.engineSymbol,1000).then((ledger)=>setCardLedgers((current)=>({...current,[item.symbol]:ledger}))).catch(()=>setCardLedgerStatus((current)=>({...current,[item.symbol]:"unavailable"}))).finally(()=>cardLedgerRequestsRef.current.delete(item.symbol));
  };
  const [viewModeBySymbol, setViewModeBySymbol] = useState<Record<string, "live" | "backtest">>({});
  const [viewModesReady, setViewModesReady] = useState(false);
  const selectedMode = selected ? (viewModeBySymbol[selected.symbol] || "live") : "live";
  useEffect(() => {
    let active = true;
    void AsyncStorage.getItem("qoin:archive-card-modes").then((raw) => {
      if (!active) return;
      if (raw) {
        try {
          const parsed: unknown = JSON.parse(raw);
          if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
            const valid = Object.fromEntries(Object.entries(parsed).filter((entry) => entry[1] === "live" || entry[1] === "backtest")) as Record<string, "live" | "backtest">;
            setViewModeBySymbol(valid);
          }
        } catch { /* Ignore malformed local preference data and use live mode. */ }
      }
      setViewModesReady(true);
    }).catch(() => { if (active) setViewModesReady(true); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    if (viewModesReady) void AsyncStorage.setItem("qoin:archive-card-modes", JSON.stringify(viewModeBySymbol)).catch(() => undefined);
  }, [viewModeBySymbol, viewModesReady]);
  const cardTouchStart = useRef<Record<string, number>>({});
  const archivedSymbolsRef = useRef<string[] | null>(null);
  const pendingArchiveActionRef = useRef<{ symbol: string; archived: boolean } | null>(null);
  useEffect(() => {
    let active = true;
    let generation = 0;
    try {
      const unsubscribe = subscribeQoinAuth((user) => {
        const requestGeneration = ++generation;
        setFirebaseUser(user);
        setAvatarUrl(null);
        archivedSymbolsRef.current = null;
        if (!user) {
          setItems((current) => current.map((item) => ({ ...item, archived: ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol) })));
          return;
        }
        void getQoinArchivedSymbols().then((symbols) => {
          if (!active || requestGeneration !== generation) return;
          const pendingAction = pendingArchiveActionRef.current;
          let nextSymbols = symbols;
          if (pendingAction?.archived && !symbols.includes(pendingAction.symbol)) nextSymbols = [...symbols, pendingAction.symbol];
          if (pendingAction && !pendingAction.archived) nextSymbols = symbols.filter((symbol) => symbol !== pendingAction.symbol);
          pendingArchiveActionRef.current = null;
          archivedSymbolsRef.current = nextSymbols;
          setItems((current) => current.map((item) => ({ ...item, archived: nextSymbols.includes(item.symbol) })));
          if (pendingAction) {
            setSelected((current) => current?.symbol === pendingAction.symbol ? { ...current, archived: pendingAction.archived } : current);
            if (nextSymbols !== symbols) void saveQoinArchivedSymbols(nextSymbols).catch((error) => Alert.alert("Archive could not be saved", error instanceof Error ? error.message : "Try again."));
          }
        }).catch((error) => {
          if (active && requestGeneration === generation) Alert.alert("Archive unavailable", error instanceof Error ? error.message : "Could not load your archive.");
        });
      });
      return () => { active = false; unsubscribe(); };
    } catch {
      return () => { active = false; };
    }
  }, []);
  const chooseAvatar = async () => {
    const permission = await ImagePicker.requestMediaLibraryPermissionsAsync();
    if (!permission.granted) { Alert.alert("Photo access needed", "Allow Qoin to access your photo library to change your profile picture."); return; }
    const selection = await ImagePicker.launchImageLibraryAsync({ mediaTypes: ImagePicker.MediaTypeOptions.Images, allowsEditing: true, aspect: [1, 1], quality: 0.85 });
    if (selection.canceled || !selection.assets[0]) return;
    setAvatarBusy(true);
    try { const response = await fetch(selection.assets[0].uri); const rawBlob = await response.blob(); const blob = new Blob([rawBlob], { type: selection.assets[0].mimeType || rawBlob.type || "image/jpeg" }); const url = await uploadQoinAvatar(blob); setAvatarUrl(url); setAccountOpen(false); }
    catch (error) { Alert.alert("Avatar upload failed", error instanceof Error ? error.message : "Try another image."); }
    finally { setAvatarBusy(false); }
  };
  const previousActionRef = useRef<Record<string, string>>({});
  const notifiedEventIdsRef = useRef<Set<string>>(new Set());
  const unreadNotifications = notifications.filter((item) => !item.read).length;
  const addNotification = (notification: QoinNotification) => setNotifications((current) => current.some((item) => item.id === notification.id) ? current : [notification, ...current].slice(0, 100));
  const archived = useMemo(() => items.filter((item) => item.archived), [items]);
  const decisionStrategy = strategy;
  const discovered = useMemo(() => items.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [items, market, query]);
  const requireAuthentication = () => {
    setAccountOpen(true);
    Alert.alert("Sign in required", "Sign in to open instrument research, change your archive or run a historical replay.", [
      { text: "Cancel", style: "cancel" },
      { text: "Sign in", onPress: () => onAuthRequested("signin") },
      { text: "Create account", onPress: () => onAuthRequested("register") },
    ]);
  };
  const openInstrument = (item: Instrument) => {
    setMechanicsTab("vector");
    setSelected(item);
  };
  const toggle = (symbol: string) => {
    const target = items.find((item) => item.symbol === symbol);
    if (!target) return;
    if (!firebaseUser) {
      pendingArchiveActionRef.current = { symbol, archived: !target.archived };
      requireAuthentication();
      return;
    }
    if (archivedSymbolsRef.current === null) {
      Alert.alert("Archive syncing", "Your saved archive is still loading. Please try again in a moment.");
      return;
    }
    const previousSymbols = archivedSymbolsRef.current;
    const nextSymbols = target.archived
      ? previousSymbols.filter((value) => value !== symbol)
      : [...previousSymbols, symbol];
    archivedSymbolsRef.current = nextSymbols;
    setItems((current) => current.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
    void saveQoinArchivedSymbols(nextSymbols).catch((error) => {
      if (archivedSymbolsRef.current === nextSymbols) {
        archivedSymbolsRef.current = previousSymbols;
        setItems((current) => current.map((item) => ({ ...item, archived: previousSymbols.includes(item.symbol) })));
        setSelected((current) => current?.symbol === symbol ? { ...current, archived: target.archived } : current);
      }
      Alert.alert("Archive could not be saved", error instanceof Error ? error.message : "Try again.");
    });
  };
  useEffect(() => { let active = true; getQoinAlgorithmCatalogue().then((data) => { if (active) { setCatalogue(data); setCatalogueStatus("loaded"); } }).catch(() => { if (active) setCatalogueStatus("unavailable"); }); return () => { active = false; }; }, []);
  useEffect(() => {
    let active = true;
    getQoinInstruments().then(({ items }) => {
      if (!active) return;
      const colors = ["#E6A23C", "#6477DF", "#8B72E8", "#4D91D8", "#76A85A", "#8895A7"];
      setItems(items.map((item, index) => ({
        symbol: item.symbol, engineSymbol: item.engine_symbol, name: item.name,
        market: item.asset_class, detail: item.description, venue: item.venue, engineVenue: item.venue_id || (item.asset_class === "Crypto" ? "binance_usdm" : "finnhub"), provider: item.provider, providerStatus: item.provider_status,
        archived: archivedSymbolsRef.current ? archivedSymbolsRef.current.includes(item.symbol) : ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol), tint: colors[index % colors.length],
      })));
    }).catch(() => undefined);
    return () => { active = false; };
  }, []);
  useEffect(() => {
    let active = true;
    const load = () => {
      setNewsStatus("loading");
      getQoinNews(newsCategory).then((result) => {
        if (!active) return;
        setNews(result.items);
        setNewsStatus(result.status === "ok" ? "connected" : result.message || result.status);
      }).catch((error: unknown) => {
        if (active) setNewsStatus(error instanceof Error ? error.message : "News provider unavailable");
      });
    };
    load();
    const timer = setInterval(load, 5 * 60 * 1000);
    return () => { active = false; clearInterval(timer); };
  }, [newsCategory]);
  useEffect(() => {
    let active = true;
    const load = () => getQoinCalendar().then((result) => {
      if (!active) return;
      setMacroEvents(result.items);
      setCalendarStatus(result.status === "ok" ? "connected" : result.message || result.status);
    }).catch((error: unknown) => { if (active) setCalendarStatus(error instanceof Error ? error.message : "Calendar provider unavailable"); });
    load();
    const timer = setInterval(load, 15 * 60 * 1000);
    return () => { active = false; clearInterval(timer); };
  }, []);
  useEffect(() => { getQoinStrategy().then(setStrategy).catch(() => setStrategy(null)); }, []);
  useEffect(() => {
    if (!selected) {
      setVector(null);
      setVectorError("");
      return;
    }
    let current = true;
    setVector(null);
    setVectorError("");
    setVectorLoading(true);
    getInstrumentVector(selected.engineSymbol).then((result) => {
      if (!current) return;
      if (result.decision_vector && result.features && result.attended_features) {
        setVector(result);
        setLiveBySymbol((current) => ({ ...current, [selected.engineSymbol.toUpperCase()]: { ...current[selected.engineSymbol.toUpperCase()], decision: result } }));
      } else {
        setVectorError(result.reason || "No persisted engine vector exists for this instrument yet.");
      }
    }).catch((error: unknown) => {
      if (current) setVectorError(error instanceof Error ? error.message : "Engine vector is unavailable.");
    }).finally(() => {
      if (current) setVectorLoading(false);
    });
    return () => { current = false; };
  }, [selected?.engineSymbol]);
  useEffect(() => {
    if (!selected) { setVectorHistory([]); setRawEvents([]); setSetLedger(null); setSetLedgerStatus("Select an instrument to inspect V2 sets."); setMarketState(null); setLastEvent(null); return; }
    let active = true;
    setVectorHistory([]);
    setRawEvents([]);
    setSetLedger(null);
    setSetLedgerStatus("Loading set memberships and temporal relations…");
    getQoinMarketState(selected.engineSymbol).then((state) => { if (active) setMarketState(state); }).catch(() => undefined);
    getQoinSetLedger(selected.engineSymbol, 1000).then((result) => { if (active) { setSetLedger(result); setSetLedgerStatus("Loaded"); } }).catch((error: unknown) => { if (active) setSetLedgerStatus(error instanceof Error ? error.message : "Set ledger unavailable."); });
    getInstrumentVectorHistory(selected.engineSymbol, 500).then((result) => { if (active) setVectorHistory(result.items); }).catch(() => undefined);
    getQoinEvents(selected.engineSymbol, 1000).then((result) => { if (active) { setRawEvents(result.items.slice().reverse()); const latest = result.items[result.items.length - 1]; setLastEvent(latest ? { event_id: latest.event_id, venue: latest.venue, symbol: latest.symbol, event_type: latest.event_type, ts_ms: latest.ts_ms, price: latest.price, size: latest.size, side: latest.side, bid: latest.bid, ask: latest.ask, bid_size: latest.bid_size, ask_size: latest.ask_size, funding_rate: latest.funding_rate, open_interest: latest.open_interest, liquidation_side: latest.liquidation_side } : null); } }).catch(() => undefined);
    return () => { active = false; };
  }, [selected?.engineSymbol]);

  useEffect(() => {
    const symbols = items.map((item) => item.engineSymbol);
    if (!symbols.length) return;
    let active = true;
    let ledgerRefreshTimer: ReturnType<typeof setTimeout> | null = null;
    let ledgerRefreshInFlight = false;
    let ledgerRefreshQueued = false;
    const refreshSetLedger = async () => {
      if (!active || !selected) return;
      if (ledgerRefreshInFlight) { ledgerRefreshQueued = true; return; }
      ledgerRefreshInFlight = true;
      try {
        const result = await getQoinSetLedger(selected.engineSymbol, 1000);
        if (active) {
          setSetLedger(result);
          setSetLedgerStatus("Live · " + new Date().toLocaleTimeString());
        }
      } catch (error) {
        if (active) setSetLedgerStatus(error instanceof Error ? "Refresh failed · " + error.message : "Set ledger refresh failed");
      } finally {
        ledgerRefreshInFlight = false;
        if (ledgerRefreshQueued && active) { ledgerRefreshQueued = false; void refreshSetLedger(); }
      }
    };
    const scheduleSetLedgerRefresh = () => {
      if (!selected || ledgerRefreshTimer) return;
      setSetLedgerStatus("New evidence · rebuilding V2 projection…");
      ledgerRefreshTimer = setTimeout(() => { ledgerRefreshTimer = null; void refreshSetLedger(); }, 450);
    };
    const unsubscribe = subscribeQoinStream(symbols, (update) => {
      if (!active) return;
      if (update.type === "market.update" && update.symbol) {
        const symbol = update.symbol.toUpperCase();
        if (update.decision?.action) {
          const action = update.decision.action.toUpperCase();
          const previous = previousActionRef.current[symbol];
          if ((action === "BUY" || action === "SELL") && previous !== action) {
            addNotification({ id: "signal-" + symbol + "-" + action + "-" + (update.server_ts_ms || Date.now()), symbol, title: symbol + " research signal: " + action, body: (update.decision.reason || "The engine decision changed.") + " Magnitude " + (update.decision.magnitude ?? 0).toFixed(3) + ". Research only; no order was placed.", kind: "signal", createdAt: update.server_ts_ms || Date.now(), read: false, details: JSON.stringify(update.decision, null, 2) });
          }
          previousActionRef.current[symbol] = action;
        }
        if (update.event && (update.event.event_type === "liquidation" || update.event.liquidation_side)) {
          const eventId = update.event.event_id || symbol + "-" + (update.event.ts_ms || update.server_ts_ms || Date.now());
          if (!notifiedEventIdsRef.current.has(eventId)) {
            notifiedEventIdsRef.current.add(eventId);
            addNotification({ id: "liquidation-" + eventId, symbol, title: symbol + " liquidation event", body: "Liquidation side: " + (update.event.liquidation_side || update.event.side || "not supplied") + ". Price: " + (update.event.price ?? "not supplied") + ".", kind: "liquidation", createdAt: update.event.ts_ms || update.server_ts_ms || Date.now(), read: false, details: JSON.stringify(update.event, null, 2) });
          }
        }
        setLiveBySymbol((current) => ({ ...current, [symbol]: { state: update.state, decision: update.decision, event: update.event } }));
        if (selected?.engineSymbol.toUpperCase() === symbol) {
          if (update.state) setMarketState(update.state);
          if (update.event) {
            setLastEvent(update.event);
            setRawEvents((current) => [{
              ...update.event,
              event_id: update.event?.event_id || String(update.server_ts_ms || Date.now()),
              venue: update.event?.venue || "stream", symbol,
              event_type: update.event?.event_type || "unknown",
              ts_ms: update.event?.ts_ms || update.server_ts_ms || Date.now(),
            } as QoinMarketEvent, ...current].slice(0, 1000));
          }
          if (update.event) scheduleSetLedgerRefresh();
          if (update.decision) {
            setVector(update.decision);
            setVectorHistory((current) => [...current.slice(-499), {
              symbol: update.decision!.symbol, decision_vector: update.decision!.decision_vector || [0, 0],
              magnitude: update.decision!.magnitude || 0, angle_radians: update.decision!.angle_radians || 0,
              action: update.decision!.action || "wait", strategy_version: update.decision!.strategy_version,
              stored_at: new Date(update.server_ts_ms || Date.now()).toISOString(),
            }]);
          }
        }
      }
      if (update.type === "stream.gap") {
        setStreamStatus("gap — resynchronizing snapshots");
        if (selected) {
          setSetLedgerStatus("Stream gap · restoring authoritative snapshots…");
          void Promise.all([
            getInstrumentVector(selected.engineSymbol),
            getInstrumentVectorHistory(selected.engineSymbol, 500),
            getQoinEvents(selected.engineSymbol, 1000),
            getQoinSetLedger(selected.engineSymbol, 1000),
          ]).then(([decision, history, events, ledger]) => {
            if (!active) return;
            if (decision.decision_vector && decision.features && decision.attended_features) setVector(decision);
            setVectorHistory(history.items);
            setRawEvents(events.items.slice().reverse());
            setSetLedger(ledger);
            setSetLedgerStatus("Resynchronized · " + new Date().toLocaleTimeString());
          }).catch((error: unknown) => {
            if (active) setSetLedgerStatus(error instanceof Error ? "Resynchronization failed · " + error.message : "Resynchronization failed");
          });
        }
      }
    }, setStreamStatus);
    return () => { active = false; if (ledgerRefreshTimer) clearTimeout(ledgerRefreshTimer); unsubscribe(); };
  }, [items, selected?.engineSymbol]);

  useEffect(() => {
    const current = selected ? liveBySymbol[selected.engineSymbol.toUpperCase()] : undefined;
    setMarketState(current?.state || null);
    setLastEvent(current?.event || null);
  }, [selected?.engineSymbol, liveBySymbol]);

  const bullish = vector?.decision_vector?.[0] ?? 0;
  const bearish = vector?.decision_vector?.[1] ?? 0;
  const vectorPoints = [...vectorHistory, ...(vector?.decision_vector ? [{ decision_vector: vector.decision_vector } as QoinVectorHistory] : [])];
  const vectorScale = Math.max(0.35, ...vectorPoints.flatMap((item) => item.decision_vector));
  const trajectoryPoints = vectorPoints.map((item) => (10 + item.decision_vector[0] / vectorScale * 80) + "," + (90 - item.decision_vector[1] / vectorScale * 80)).join(" ");
  const vectorX = 10 + (bullish / vectorScale) * 80;
  const vectorY = 90 - (bearish / vectorScale) * 80;

  const renderPaneContent = (paneMode: MainTab) => {
    const paneData = paneMode === "Archive" ? archived : discovered;
    return (
      <>
        
        <View style={styles.headingRow}><View><Text style={styles.kicker}>{paneMode === "Archive" ? "YOUR COLLECTION" : "DISCOVERY"}</Text><Text style={styles.heading}>{paneMode}</Text><Text style={styles.subheading}>{paneMode === "Archive" ? "Every instrument you follow, in one place." : "Find instruments to bring into your archive."}</Text></View><Text style={styles.count}>{paneMode === "Archive" ? archived.length : items.length - archived.length} ASSETS</Text></View>

        {paneMode === "Explore" ? (
          <>
            <View style={styles.search}><Text style={styles.searchIcon}>⌕</Text><TextInput value={query} onChangeText={setQuery} placeholder="Search instruments or symbols" placeholderTextColor="#89968D" style={styles.searchInput} /></View>
            <View style={styles.coverageSection}>
              <View style={styles.coverageHeading}><View style={styles.coverageHeadingCopy}><Text style={styles.kicker}>MARKET ACCESS DESIGN</Text><Text style={styles.newsTitle}>Coverage tiers</Text><Text style={styles.coverageSubheading}>Data breadth is separate from strategy performance and broker execution.</Text></View><Text style={styles.coverageStatus}>{catalogueStatus === "loaded" ? (catalogue?.market_access_tiers_enabled ? "ENABLED" : "PREVIEW") : catalogueStatus.toUpperCase()}</Text></View>
              {catalogue?.market_access_tiers?.length ? catalogue.market_access_tiers.map((tier) => <View key={tier.tier_id} style={styles.coverageCard}>
                <View style={styles.coverageCardTop}><Text style={styles.coverageName}>{tier.display_name}</Text><Text style={styles.coverageTag}>{tier.status}</Text></View>
                <Text style={styles.coveragePrimary}>{tier.max_active_instruments} <Text style={styles.coverageUnit}>active instruments</Text></Text>
                <Text style={styles.coverageLine}>Markets · {tier.asset_classes.join(" · ")}</Text>
                <Text style={styles.coverageLine}>History · {tier.historical_depth_days} days</Text>
                <Text style={styles.coverageLine}>Timeframes · {tier.supported_timeframes.join(" · ")}</Text>
                <Text style={styles.coverageLine}>Providers · {tier.market_data_providers.join(" · ") || "None listed"}</Text>
                <Text style={styles.coverageLine}>Execution brokers · {tier.execution_brokers.length ? tier.execution_brokers.join(" · ") : "None — research only"}</Text>
                <Text style={styles.coverageNote}>{tier.notes}</Text>
              </View>) : <Text style={styles.noteCopy}>{catalogueStatus === "loading" ? "Loading proposed market coverage…" : "Market coverage catalogue unavailable."}</Text>}
              <Text style={styles.coverageDisclaimer}>Preview only: not active subscriptions, prices or enforced permissions. Finnhub data depends on a configured key and eligible access. No live broker execution is connected.</Text>
            </View>
            <MobileBrokerCollectionPanel />
          </>
        ) : null}

        {paneData.length ? paneData.map((item) => {
          const live=liveBySymbol[item.engineSymbol.toUpperCase()];
          const decision=live?.decision;
          const page=algorithmBySymbol[item.symbol]??0;
          const cardLedger=cardLedgers[item.symbol];
          const ledgerStatus=cardLedgerStatus[item.symbol]||"idle";
          const setPage=(next:0|1)=>{setAlgorithmBySymbol((current)=>({...current,[item.symbol]:next}));if(next===1)loadCardLedger(item);};
          const sourceTime=decision?.input_event?.event_ts_ms;
          return <View key={item.symbol} style={styles.card}>
            <View onTouchStart={(event)=>{cardTouchStart.current[item.symbol]=event.nativeEvent.pageX;}} onTouchEnd={(event)=>{const start=cardTouchStart.current[item.symbol],end=event.nativeEvent.pageX;if(start!=null&&Math.abs(end-start)>38)setPage(end<start?1:0);delete cardTouchStart.current[item.symbol];}}>
              <Pressable onPress={()=>openInstrument(item)} style={styles.cardMain}>
                <View style={[styles.assetMark,{backgroundColor:item.tint}]}><Text style={styles.assetMarkText}>{item.symbol.slice(0,1)}</Text></View>
                <View style={styles.assetCopy}><Text style={styles.assetName}>{item.name}</Text><Text style={styles.assetMeta}>{item.symbol} · {item.market}</Text></View>
                <Text style={styles.arrow}>↗</Text>
              </Pressable>
              <View style={styles.cardMetaRow}>
                <Text style={styles.cardMetaChip}>{item.market}</Text><Text style={styles.cardMetaChip}>{item.venue}</Text>
                <Text style={[styles.cardMetaChip,item.providerStatus?.toLowerCase().includes("connected")&&styles.cardMetaLive]}>{item.providerStatus||"Provider unknown"}</Text>
                <Text style={styles.cardMetaChip}>PRICE · {live?.state?.last_price?.toLocaleString(undefined,{maximumFractionDigits:8})??"—"}</Text>
              </View>
              {page===0?<View style={[styles.cardPager,{flexDirection:"column",alignItems:"stretch",gap:8}]}>
                <View style={{flexDirection:"row",alignItems:"center",justifyContent:"space-between",flexWrap:"wrap"}}><Text style={styles.cardKicker}>RESEARCH MODEL 01</Text><Text style={styles.cardPagerStatus}>{decision?.action?.toUpperCase()||"AWAITING DATA"}</Text></View>
                <View style={{flexDirection:"row",alignItems:"baseline",gap:8}}><Text style={styles.cardPagerValue}>{decision?.magnitude?.toFixed(4)??"—"}</Text><Text style={styles.cardMetaLabel}>DECISION MAGNITUDE</Text></View>
                <View style={{flexDirection:"row",justifyContent:"space-between",gap:8,flexWrap:"wrap"}}><Text style={styles.cardMetaLabel}>RESULTANT VECTOR · [BULL, BEAR]</Text><Text style={styles.cardMetaValue}>{decision?.decision_vector?.[0]?.toFixed(4)??"—"} / {decision?.decision_vector?.[1]?.toFixed(4)??"—"}</Text></View>
                <View style={{flexDirection:"row",flexWrap:"wrap",gap:6}}>{QOIN_FEATURE_NAMES.map((name)=><View key={name} style={{width:"31%",minWidth:78,padding:7,borderRadius:8,backgroundColor:"#0E1612"}}><Text style={[styles.cardMetaLabel,{fontSize:8}]}>{name.replaceAll("_"," ")}</Text><Text style={styles.cardMetaValue}>{decision?.features?.[name]?.toFixed(3)??"—"}</Text></View>)}</View>
                <Text style={styles.cardReasonText}>{decision?.reason||"No persisted vector decision is available. No preview values are substituted."}</Text>
                <Text style={styles.cardMetaLabel}>{decision?.strategy_version||"Strategy version unavailable"} · {sourceTime?new Date(sourceTime).toLocaleString():"No source event timestamp"}</Text>
              </View>:<View style={[styles.cardPager,{flexDirection:"column",alignItems:"stretch",gap:9}]}>
                <View style={{flexDirection:"row",alignItems:"center",justifyContent:"space-between"}}><Text style={styles.cardKicker}>RESEARCH MODEL 02</Text><Text style={styles.cardPagerStatus}>{cardLedger?"EVIDENCE LOADED":ledgerStatus==="loading"?"LOADING":ledgerStatus==="unavailable"?"UNAVAILABLE":"ON DEMAND"}</Text></View>
                {cardLedger?<><View style={{flexDirection:"row",flexWrap:"wrap",gap:8}}>{[["UNIVERSE",cardLedger.universe_count],["SETS",cardLedger.sets.length],["RELATIONS",cardLedger.relations.length]].map(([label,value])=><View key={String(label)} style={{flex:1,minWidth:72,padding:9,borderRadius:9,backgroundColor:"#0E1612"}}><Text style={styles.cardMetaLabel}>{label}</Text><Text style={styles.cardPagerValue}>{value}</Text></View>)}</View>
                  {cardLedger.sets.slice(0,5).map((set)=><View key={set.set_id} style={{flexDirection:"row",alignItems:"center",gap:7}}><Text numberOfLines={1} style={{width:105,color:"#C5D0C7",fontSize:8}}>{set.label}</Text><View style={{flex:1,height:7,borderRadius:6,backgroundColor:"#243B28",overflow:"hidden"}}><View style={{height:7,width:percentageWidth(set.measure.counting_measure,cardLedger.universe_count),backgroundColor:"#7DF162"}}/></View><Text style={{width:22,textAlign:"right",fontSize:9,color:"#EDF4EE"}}>{set.measure.counting_measure}</Text></View>)}
                  <Text style={styles.cardMetaLabel}>{cardLedger.universe_truncated?"Evidence sample is capped.":"Membership is derived from persisted event IDs."}</Text></>:<View style={{gap:7}}><Text style={styles.cardReasonText}>{ledgerStatus==="loading"?"Loading persisted event memberships and measures…":ledgerStatus==="unavailable"?"V2 ledger evidence is unavailable for this instrument.":"Swipe to load the set and ledger model."}</Text>{ledgerStatus==="unavailable"?<Pressable onPress={()=>{setCardLedgerStatus((current)=>{const next={...current};delete next[item.symbol];return next;});loadCardLedger(item);}} style={styles.addButton}><Text style={styles.addText}>Retry evidence load</Text></Pressable>:null}</View>}
              </View>}
              <View style={styles.cardPagerFooter}><View style={styles.cardPagerDots}>
                <Pressable accessibilityLabel="Show research model one" accessibilityState={{selected:page===0}} onPress={()=>setPage(0)} style={[styles.cardPagerDot,page===0&&styles.cardPagerDotActive]}/>
                <Pressable accessibilityLabel="Show research model two" accessibilityState={{selected:page===1}} onPress={()=>setPage(1)} style={[styles.cardPagerDot,page===1&&styles.cardPagerDotActive]}/>
              </View><Text style={styles.cardSwipeHint}>SWIPE · TWO MODELS</Text></View>
            </View>
            <View style={{flexDirection:"row",alignItems:"center",gap:8,marginTop:9,marginLeft:48,flexWrap:"wrap"}}>
              <Pressable accessibilityRole="button" accessibilityState={{selected:(viewModeBySymbol[item.symbol]||"live")==="backtest"}} onPress={()=>setViewModeBySymbol((current)=>({...current,[item.symbol]:current[item.symbol]==="backtest"?"live":"backtest"}))} style={{paddingVertical:8,paddingHorizontal:11,borderRadius:10,backgroundColor:"#243B28",borderWidth:0,borderColor:"transparent"}}><Text style={{color:"#7DF162",fontSize:9,fontWeight:"800"}}>{(viewModeBySymbol[item.symbol]||"live")==="live"?"Switch to Backtest":"Switch to Live"}</Text></Pressable>
              {paneMode==="Explore"?<Pressable onPress={()=>toggle(item.symbol)} style={styles.addButton}><Text style={styles.addText}>＋ Add to archive</Text></Pressable>:null}
            </View>
          </View>;
        }) : <View style={styles.empty}><Text style={styles.emptyTitle}>Nothing here yet.</Text><Text style={styles.emptyCopy}>{paneMode==="Archive"?"Explore instruments and add the ones you want to follow.":"Try another name or market."}</Text></View>}

        {paneMode === "Explore" ? <View style={styles.newsSection}>
          <Text style={styles.kicker}>GLOBAL FINANCIAL INTELLIGENCE</Text><Text style={styles.newsTitle}>News & macro events</Text>
          <Text style={styles.noteCopy}>Provider-attributed headlines with publication timestamps and original links.</Text>
          <View style={styles.filters}>{["general", "forex", "crypto", "merger"].map((category) => <Pressable key={category} onPress={() => setNewsCategory(category)} style={[styles.filter, newsCategory === category && styles.filterActive]}><Text style={[styles.filterText, newsCategory === category && styles.filterTextActive]}>{category}</Text></Pressable>)}</View>
          {news.length ? news.map((item) => <Pressable key={String(item.id)} style={styles.newsItem} onPress={() => Linking.openURL(item.url).catch(() => undefined)}>
            <Text style={styles.newsMeta}>{item.source} · {item.published_at_ms ? new Date(item.published_at_ms).toLocaleString() : "Timestamp unavailable"}</Text>
            <Text style={styles.newsHeadline}>{item.headline}</Text><Text style={styles.newsSummary} numberOfLines={3}>{item.summary}</Text>
          </Pressable>) : <Text style={styles.noteCopy}>{newsStatus === "loading" ? "Loading sourced headlines…" : newsStatus + ". Configure FINNHUB_API_KEY on the backend. No placeholder headlines are generated."}</Text>}
        </View> : null}
        {paneMode === "Explore" ? <View style={styles.macroSection}>
          <Text style={styles.kicker}>UPCOMING CATALYSTS</Text><Text style={styles.newsTitle}>Economic calendar</Text>
          <Text style={styles.noteCopy}>Scheduled releases, with actual, estimate and previous values where published.</Text>
          {macroEvents.length ? macroEvents.map((item,index) => <View key={item.time + item.country + item.event + index} style={styles.macroItem}>
            <Text style={styles.newsHeadline}>{item.event}</Text><Text style={styles.newsMeta}>{item.country} · {item.time} · {item.impact || "Impact not supplied"}</Text>
            <View style={styles.macroValues}><Text style={styles.newsMeta}>Actual: {item.actual ?? "—"}</Text><Text style={styles.newsMeta}>Estimate: {item.estimate ?? "—"}</Text><Text style={styles.newsMeta}>Previous: {item.previous ?? "—"}</Text></View>
          </View>) : <Text style={styles.noteCopy}>{calendarStatus === "loading" ? "Loading economic calendar…" : calendarStatus + ". Configure FINNHUB_API_KEY to load macro events. No event dates are fabricated."}</Text>}
        </View> : null}

        <Text style={styles.footer}>QOIN · RESEARCH MODE · NO LIVE EXECUTION</Text>

      </>
    );
  };

  return (
    <SafeAreaView style={styles.screen} edges={["top"]}>
      <Animated.View pointerEvents={headerVisible ? "auto" : "none"} style={[styles.topline, { paddingHorizontal: 17, opacity: headerVisible ? 1 : 0, transform: [{ translateY: headerTranslateY }] }]}>
        <Text style={styles.brand}>qoin</Text>
        <View style={styles.topActions}>
          <Pressable accessibilityRole="button" accessibilityLabel="Open menu" onPress={() => setAccountOpen(true)} style={styles.notificationButton}>
            <Svg width={20} height={20} viewBox="0 0 24 24">
              <Path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="#C5D0C7" strokeWidth={1.8} strokeLinecap="round" />
            </Svg>
          </Pressable>
          <Pressable accessibilityRole="button" accessibilityLabel="Open account menu" onPress={() => setAccountOpen(true)} style={styles.avatar}>{(avatarUrl || firebaseUser?.photoURL) ? <Image source={{ uri: avatarUrl || firebaseUser?.photoURL || "" }} style={styles.avatarImage} /> : <Text style={styles.avatarText}>{(firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</Text>}</Pressable>
        </View>
      </Animated.View>
      <View style={{ flex: 1 }}>
        <ScrollView ref={archiveScrollRef} style={{ flex: 1, display: mode === "Archive" ? "flex" : "none" }} contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled" onScroll={(event) => handleWorkspaceScroll("Archive", event)} scrollEventThrottle={16}>
          {renderPaneContent("Archive")}
        </ScrollView>
        {visitedModes.Explore ? <ScrollView ref={exploreScrollRef} style={{ flex: 1, display: mode === "Explore" ? "flex" : "none" }} contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled" onScroll={(event) => handleWorkspaceScroll("Explore", event)} scrollEventThrottle={16}>
          {renderPaneContent("Explore")}
        </ScrollView> : null}
      </View>

      <Modal visible={accountOpen} transparent animationType="fade" onRequestClose={() => setAccountOpen(false)}>
        <Pressable style={styles.accountShade} onPress={() => setAccountOpen(false)}><Pressable style={styles.accountSheet} onPress={(event) => event.stopPropagation()}>
          <View style={styles.accountIdentity}><View style={styles.accountAvatar}>{(avatarUrl || firebaseUser?.photoURL) ? <Image source={{ uri: avatarUrl || firebaseUser?.photoURL || "" }} style={styles.accountAvatarImage} /> : <Text style={styles.avatarText}>{(firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</Text>}</View><View style={{ flex: 1 }}><Text style={styles.accountName}>{firebaseUser?.displayName || (firebaseUser ? "Qoin member" : "Guest")}</Text><Text style={styles.accountEmail}>{firebaseUser?.email || (firebaseUser ? "Signed in" : "Browse Qoin without an account")}</Text></View></View>
          {firebaseUser ? <>
            <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); setProfileOpen(true); }}><Text style={styles.accountActionText}>Profile</Text></Pressable>
            <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); setNotificationsOpen(true); }}><Text style={styles.accountActionText}>Notifications {unreadNotifications ? "· " + unreadNotifications + " unread" : ""}</Text></Pressable>
            <Pressable style={styles.accountAction} disabled={avatarBusy} onPress={() => void chooseAvatar()}><Text style={styles.accountActionText}>{avatarBusy ? "Uploading picture…" : "Change profile picture"}</Text></Pressable>
            <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); void signOutQoinUser().catch((error) => Alert.alert("Sign out failed", error instanceof Error ? error.message : "Try again.")); }}><Text style={styles.accountActionText}>Sign out</Text></Pressable>
          </> : <>
            <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); onAuthRequested("signin"); }}><Text style={styles.accountActionText}>Sign in</Text></Pressable>
            <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); onAuthRequested("register"); }}><Text style={styles.accountActionText}>Create account</Text></Pressable>
          </>}
        </Pressable></Pressable>
      </Modal>
      <Modal visible={profileOpen} transparent animationType="fade" onRequestClose={() => setProfileOpen(false)}>
        <View style={styles.modalShade}><View style={styles.profileModal}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>YOUR ACCOUNT</Text><Text style={styles.modalTitle}>Profile</Text></View><Pressable onPress={() => setProfileOpen(false)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View>
          <View style={styles.profileIdentity}><View style={styles.profileAvatar}>{(avatarUrl || firebaseUser?.photoURL) ? <Image source={{ uri: avatarUrl || firebaseUser?.photoURL || "" }} style={styles.accountAvatarImage} /> : <Text style={styles.avatarText}>{(firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</Text>}</View><View style={{ flex: 1 }}><Text style={styles.accountName}>{firebaseUser?.displayName || "Qoin member"}</Text><Text style={styles.accountEmail}>{firebaseUser?.email || ""}</Text></View></View>
          <Text style={styles.profileUidLabel}>FIREBASE USER ID</Text><Text selectable style={styles.profileUid}>{firebaseUser?.uid || ""}</Text>
          <Pressable style={styles.backtestRun} onPress={() => { setProfileOpen(false); setAccountOpen(true); }}><Text style={styles.backtestRunText}>Account options</Text></Pressable>
        </View></View>
      </Modal>
      <Modal visible={notificationsOpen} transparent animationType="slide" onRequestClose={() => setNotificationsOpen(false)}>
        <View style={styles.modalShade}><View style={styles.notificationModal}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>LIVE ENGINE EVENTS</Text><Text style={styles.modalTitle}>Notifications</Text><Text style={styles.assetMeta}>{unreadNotifications} unread · current session</Text></View><Pressable onPress={() => setNotificationsOpen(false)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View>
          <View style={styles.notificationActions}><Pressable onPress={() => setNotifications((current) => current.map((item) => ({ ...item, read: true })))}><Text style={styles.notificationActionText}>Mark all read</Text></Pressable><Pressable onPress={() => setNotifications([])}><Text style={styles.notificationActionText}>Clear history</Text></Pressable></View>
          <ScrollView contentContainerStyle={styles.notificationContent}>{notifications.length ? notifications.map((item) => <View key={item.id} style={[styles.notificationItem, item.read && styles.notificationRead]}><Text style={styles.notificationMeta}>{item.kind.toUpperCase()} · {item.symbol} · {new Date(item.createdAt).toLocaleTimeString()}</Text><Text style={styles.notificationTitle}>{item.title}</Text><Text style={styles.notificationBody}>{item.body}</Text><Text style={styles.notificationPayload}>{item.details}</Text></View>) : <Text style={styles.noteCopy}>No alerts yet. Signal transitions and liquidation events from the live stream will appear here.</Text>}</ScrollView>
        </View></View>
      </Modal>
      <Modal visible={!!selected} transparent={false} animationType="slide" presentationStyle="fullScreen" onRequestClose={() => setSelected(null)}>
        {selected ? <SafeAreaView style={styles.modalShade}><View style={styles.modalCard}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>{selectedMode === "backtest" ? "BACKTEST ARENA" : "INSTRUMENT SPACE"}</Text><Text style={styles.modalTitle}>{selectedMode === "backtest" ? selected.name + " · Backtest" : selected.name}</Text><Text style={styles.assetMeta}>{selected.symbol} · {selected.market}</Text></View><View style={{ flexDirection: "row", alignItems: "center", gap: 7 }}><Pressable onPress={() => setViewModeBySymbol((current) => ({ ...current, [selected.symbol]: selectedMode === "live" ? "backtest" : "live" }))} style={{ paddingVertical: 8, paddingHorizontal: 10, borderRadius: 9, backgroundColor: "#243B28" }}><Text style={{ color: "#7DF162", fontSize: 9, fontWeight: "800" }}>{selectedMode === "live" ? "Backtest" : "Live"}</Text></Pressable><Pressable onPress={() => setSelected(null)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View></View>
          <ScrollView contentContainerStyle={styles.modalContent}>
            {selectedMode === "live" ? <View style={styles.mechanicsTabs}>
              <Pressable accessibilityRole="tab" accessibilityState={{ selected: mechanicsTab === "vector" }} style={[styles.mechanicsTab, mechanicsTab === "vector" && styles.mechanicsTabActive]} onPress={() => setMechanicsTab("vector")}><Text style={[styles.mechanicsTabText, mechanicsTab === "vector" && styles.mechanicsTabTextActive]}>Vector space</Text></Pressable>
              <Pressable accessibilityRole="tab" accessibilityState={{ selected: mechanicsTab === "sets" }} style={[styles.mechanicsTab, mechanicsTab === "sets" && styles.mechanicsTabActive]} onPress={() => setMechanicsTab("sets")}><Text style={[styles.mechanicsTabText, mechanicsTab === "sets" && styles.mechanicsTabTextActive]}>Sets &amp; Ledger</Text></Pressable>
            </View> : null}
            {selectedMode === "backtest" ? <View style={{ gap: 14 }}>
              <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Backtest mechanics</Text><Text style={styles.attentionCaption}>Historical V1 replay · deterministic event order · no orders</Text></View>
              <View style={styles.tracePanel}>
                <Text style={styles.attentionTitle}>How the replay progresses</Text>
                {[
                  ["01 · Historical tape", "Load stored observations for this instrument and venue, ordered by event timestamp and event ID."],
                  ["02 · State reconstruction", "Rebuild the same V1 state and normalized feature calculations used by the live research engine."],
                  ["03 · Decision evaluation", "Apply the pinned strategy version and gates; record BUY, SELL and WAIT decisions."],
                  ["04 · Simulated execution", "Fill only on a strictly later eligible trade or book event. Buys use ask, sells use bid; latency and liquidity participation constrain fills."],
                  ["05 · Cost accounting", "Apply fees, slippage, market impact and recorded funding cashflows to equity and realized P/L."],
                  ["06 · Audit", "Inspect fills, pending orders, event gaps, assumptions and cost-stress scenarios. Nothing is sent to a broker."],
                ].map(([title, description]) => <View key={title} style={{gap:3}}><Text style={styles.engineValue}>{title}</Text><Text style={styles.attentionCaption}>{description}</Text></View>)}
                <Text style={styles.disclaimer}>Only persisted observations are replayed. Missing history remains missing; Qoin reports data gaps rather than fabricating candles or prices.</Text>
              </View>
              <View style={styles.mapWrap}>
                <Svg viewBox="0 0 100 100" width="100%" height={220}>
                  <Rect x="0" y="0" width="100" height="100" fill="#ffffff" />
                  {Array.from({ length: 9 }, (_, i) => <Line key={"bg-v-" + i} x1={i * 12.5} y1="0" x2={i * 12.5} y2="100" stroke="#e6edf7" strokeWidth=".4" />)}
                  {Array.from({ length: 9 }, (_, i) => <Line key={"bg-h-" + i} x1="0" y1={i * 12.5} x2="100" y2={i * 12.5} stroke="#e6edf7" strokeWidth=".4" />)}
                  <Line x1="10" y1="8" x2="10" y2="90" stroke="#7b91b5" strokeWidth=".7" /><Line x1="10" y1="90" x2="94" y2="90" stroke="#7b91b5" strokeWidth=".7" />
                  {vectorPoints.length > 1 ? <Polyline points={trajectoryPoints} fill="none" stroke="#3475e8" strokeWidth="1.2" /> : null}
                  {vector?.decision_vector ? <><Line x1="10" y1="90" x2={vectorX} y2={vectorY} stroke="#3475e8" strokeWidth="1.8" /><Circle cx={vectorX} cy={vectorY} r="3.8" fill="#3475e8" stroke="#ffffff" strokeWidth="1.2" /></> : <SvgText x="50" y="50" fill="#7b8798" fontSize="5" textAnchor="middle">NO VECTOR YET</SvgText>}
                </Svg>
              </View>
              <Text style={styles.mapCopy}>Horizontal · bullish component. Vertical · bearish component. {vectorHistory.length} persisted decisions.</Text>
              <FeatureSpace decision={vector} />
              {vector ? <DecisionTracePanel decision={vector} /> : <Text style={styles.disclaimer}>{vectorError || "No persisted vector is available yet; the authoritative seven-force Rust engine has not emitted a decision yet."}</Text>}
              <MobileResearchLedger decision={vector} history={vectorHistory} events={rawEvents} />
              <MobileSetLedgerPanel ledger={setLedger} status={setLedgerStatus} />
              <MobileBacktestPanel symbol={selected.engineSymbol} venue={selected.engineVenue} authenticated={Boolean(firebaseUser)} onRequireAuth={requireAuthentication} />
            </View> : mechanicsTab === "vector" ? <>
            <Text style={styles.mapTitle}>Vector space · seven engine dimensions</Text><Text style={styles.mapCopy}>2D projection of actual normalized R⁷ feature state with attention interactions.</Text>
            <MobileVectorSpaceMechanics decision={vector} marketState={marketState} lastEvent={lastEvent} />
            <Text style={styles.mapTitle}>Resultant decision space</Text><Text style={styles.mapCopy}>Persisted bullish and bearish output vector and its historical trajectory.</Text>
            <View style={styles.mapWrap}>
              <Svg viewBox="0 0 100 100" width="100%" height={270}>
                <Rect x="0" y="0" width="100" height="100" fill="#161F1A" />
                {Array.from({ length: 9 }, (_, i) => <Line key={`v${i}`} x1={i * 12.5} y1="0" x2={i * 12.5} y2="100" stroke="#17291F" strokeWidth=".4" />)}
                {Array.from({ length: 9 }, (_, i) => <Line key={`h${i}`} x1="0" y1={i * 12.5} x2="100" y2={i * 12.5} stroke="#17291F" strokeWidth=".4" />)}
                <Line x1="10" y1="8" x2="10" y2="90" stroke="#7DF162" strokeWidth=".7" /><Line x1="10" y1="90" x2="94" y2="90" stroke="#7DF162" strokeWidth=".7" />
                {vectorPoints.length > 1 ? <Polyline points={trajectoryPoints} fill="none" stroke="#7DF162" strokeWidth="1.1" /> : null}
                {vector?.decision_vector ? <><Line x1="10" y1="90" x2={vectorX} y2={vectorY} stroke="#7DF162" strokeWidth="1.8" /><Circle cx={vectorX} cy={vectorY} r="3.8" fill="#7DF162" stroke="#161F1A" strokeWidth="1.2" /></> : null}
                <Circle cx="10" cy="90" r="2" fill="#94A3B8" />
              </Svg>
            </View>
            <Text style={styles.mapCopy}>Horizontal: bullish · Vertical: bearish · {vectorHistory.length} stored observations</Text>
            <View style={styles.livePanel}>
              <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Live market state</Text><Text style={styles.attentionCaption}>{streamStatus}</Text></View>
              <View style={styles.liveGrid}>
                {[
                  ["Last price", marketState?.last_price?.toString() ?? "—"],
                  ["Spread (bps)", marketState?.spread_bps?.toFixed(3) ?? "—"],
                  ["Signed flow", marketState?.signed_flow?.toFixed(4) ?? "—"],
                  ["Order flow", marketState?.order_flow_imbalance?.toFixed(4) ?? "—"],
                  ["Funding", marketState?.funding_rate?.toFixed(6) ?? "—"],
                  ["Open interest", marketState?.open_interest?.toString() ?? "—"],
                  ["Liquidations", marketState?.liquidation_events?.toString() ?? "—"],
                  ["Last liquidation side", marketState?.last_liquidation_side ?? "—"],
                  ["Events", marketState?.event_count?.toString() ?? "—"],
                  ["Total trade size", marketState?.total_trade_size?.toFixed(4) ?? "—"],
                  ["Event intensity", marketState?.event_intensity?.toFixed(3) ?? "—"],
                  ["EWMA abs return", marketState?.ewma_abs_return?.toFixed(6) ?? "—"],
                  ["Last event time", marketState?.last_event_ts_ms ? new Date(marketState.last_event_ts_ms).toLocaleTimeString() : "—"],
                  ["State quality", marketState?.state_quality?.toFixed(3) ?? "—"],
                ].map(([label,value]) => <View key={label} style={styles.liveMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
              </View>
              <Text style={styles.disclaimer}>Last raw event: {lastEvent ? JSON.stringify(lastEvent) : "none received"}. Missing values mean the provider has not supplied that field.</Text>
            </View>
<FeatureSpace decision={vector} />
            {vector ? <DecisionTracePanel decision={vector} /> : null}
            <MobileResearchLedger decision={vector} history={vectorHistory} events={rawEvents} />
            <View style={styles.strategyPanel}>
              <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Legacy edge map · not used by active vector engine</Text><Text style={styles.attentionCaption}>{decisionStrategy?.version || "Unavailable"}</Text></View>
              {decisionStrategy ? <>
                <View style={styles.strategyGates}>
                  {[
                    ["Angle", decisionStrategy.angular_threshold_degrees.toFixed(2) + "°"],
                    ["Magnitude", decisionStrategy.magnitude_threshold.toFixed(3)],
                    ["Max spread", decisionStrategy.max_spread_bps.toFixed(2) + " bps"],
                    ["Attention", decisionStrategy.attention_scale.toFixed(3)],
                  ].map(([label,value]) => <View key={label} style={styles.liveMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
                </View>
                {QOIN_FEATURE_NAMES.map((name) => <View key={name} style={styles.strategyWeightRow}>
                  <Text style={styles.metricLabel}>{name.replaceAll("_"," ")}</Text>
                  <View style={styles.strategyWeightValues}>
                    <Text style={styles.attentionCaption}>Bullish {decisionStrategy.bullish_weights[name]?.toFixed(3) ?? "—"}</Text>
                    <Text style={styles.attentionCaption}>Bearish {decisionStrategy.bearish_weights[name]?.toFixed(3) ?? "—"}</Text>
                    <Text style={styles.attentionCaption}>Scale {decisionStrategy.feature_scales[name]?.toFixed(3) ?? "—"}</Text>
                  </View>
                </View>)}
                <Text style={styles.disclaimer}>Legacy edge-map status: {decisionStrategy.status}. The seven-force engine uses its own versioned configuration. Last update: {new Date(decisionStrategy.updated_at_ms).toLocaleString()}.</Text>
              </> : <Text style={styles.disclaimer}>The legacy edge-map compatibility settings could not be loaded.</Text>}
            </View>
            {vectorLoading ? <Text style={styles.disclaimer}>Loading the latest persisted engine vector…</Text> : vector ? (
              <>
                <View style={styles.engineMetrics}>
                  <View><Text style={styles.engineLabel}>Decision</Text><Text style={styles.engineValue}>{vector.action?.toUpperCase() || "WAIT"}</Text></View>
                  <View><Text style={styles.engineLabel}>Magnitude</Text><Text style={styles.engineValue}>{vector.magnitude?.toFixed(3) ?? "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Angle</Text><Text style={styles.engineValue}>{vector.angle_radians?.toFixed(3) ?? "—"} rad</Text></View>
                  <View><Text style={styles.engineLabel}>Strategy</Text><Text style={styles.engineValue}>{vector.strategy_version || "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Bullish component</Text><Text style={styles.engineValue}>{vector.decision_vector?.[0].toFixed(3) ?? "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Bearish component</Text><Text style={styles.engineValue}>{vector.decision_vector?.[1].toFixed(3) ?? "—"}</Text></View>
                </View>
                {QOIN_FEATURE_NAMES.map((name, index) => {
                  const raw = vector.features?.[name];
                  const attended = vector.attended_features?.[index];
                  const weights = vector.attention?.[index] ?? [];
                  const strongest = weights.length ? weights.indexOf(Math.max(...weights)) : -1;
                  return <View key={name} style={styles.metric}>
                    <Text style={styles.metricLabel}>{name.replaceAll("_", " ")}</Text>
                    <Text style={styles.metricValue}>{raw?.toFixed(3) ?? "—"} → {attended?.toFixed(3) ?? "—"}</Text>
                    {strongest >= 0 ? <Text style={styles.attentionHint}>Strongest attention: {QOIN_FEATURE_NAMES[strongest]} ({weights[strongest].toFixed(2)})</Text> : null}
                  </View>;
                })}
                <View style={styles.contributionPanel}>
                  <View style={styles.attentionHeader}><Text style={styles.attentionTitle}>Feature contribution ledger</Text><Text style={styles.attentionCaption}>Directional weight × attended force</Text></View>
                  <Text style={styles.disclaimer}>OFI × |OI change| interaction: {vector.interaction_contribution?.toFixed(5) ?? "—"}</Text>
                  {QOIN_FEATURE_NAMES.map((name,index) => {
                    const attended = vector.attended_features?.[index] ?? 0;
                    const trace = vector.feature_trace?.find((item) => item.name === name);
                    const bullishWeight = trace?.bullish_weight;
                    const bearishWeight = trace?.bearish_weight;
                    return <View key={name} style={styles.contributionRow}>
                      <Text style={styles.metricLabel}>{name.replaceAll("_"," ")}</Text>
                      <View style={styles.strategyWeightValues}>
                        <Text style={styles.attentionCaption}>{bullishWeight == null ? "Bullish weight unavailable" : "Bull " + bullishWeight.toFixed(3) + " × " + attended.toFixed(3) + " = " + (bullishWeight*attended).toFixed(4)}</Text>
                        <Text style={styles.attentionCaption}>{bearishWeight == null ? "Bearish weight unavailable" : "Bear " + bearishWeight.toFixed(3) + " × " + attended.toFixed(3) + " = " + (bearishWeight*attended).toFixed(4)}</Text>
                      </View>
                    </View>;
                  })}
                  <Text style={styles.disclaimer}>Directional feature terms and the OFI × absolute OI-change interaction are shown separately before score clipping. The actual decision vector remains authoritative.</Text>
                </View>
                <View style={styles.attentionPanel}>
                  <View style={styles.attentionHeader}><Text style={styles.attentionTitle}>Attention matrix</Text><Text style={styles.attentionCaption}>Row → column weights</Text></View>
                  <View style={styles.attentionRow}>
                    <Text style={styles.attentionRowLabel} />
                    {QOIN_FEATURE_SHORT_NAMES.map((name) => <Text key={`head-${name}`} style={styles.attentionAxis}>{name}</Text>)}
                  </View>
                  {QOIN_FEATURE_NAMES.map((rowName, rowIndex) => (
                    <View key={rowName} style={styles.attentionRow}>
                      <Text style={styles.attentionRowLabel}>{QOIN_FEATURE_SHORT_NAMES[rowIndex]}</Text>
                      {QOIN_FEATURE_NAMES.map((columnName, columnIndex) => {
                        const weight = vector.attention?.[rowIndex]?.[columnIndex] ?? 0;
                        return <View key={columnName} style={[styles.attentionCell, { backgroundColor: `rgba(112,217,157,${0.08 + weight * 0.82})` }]}>
                          <Text style={[styles.attentionValue, { color: weight > 0.55 ? "#161F1A" : "#2B4160" }]}>{weight.toFixed(2)}</Text>
                        </View>;
                      })}
                    </View>
                  ))}
                </View>
                <Text style={styles.disclaimer}>{vector.reason || "Engine research output. Not an order or a profitability claim."}</Text>
              </>
            ) : <Text style={styles.disclaimer}>{vectorError || "No persisted engine vector is available for this symbol yet."}</Text>}
            <View style={styles.rawTape}>
              <Pressable style={styles.rawTapeToggle} onPress={() => setShowRawEvents((value) => !value)}>
                <Text style={styles.attentionTitle}>Raw event tape · {rawEvents.length} observations</Text><Text style={styles.attentionCaption}>{showRawEvents ? "Hide" : "Show"}</Text>
              </Pressable>
              {showRawEvents ? rawEvents.slice(0,1000).map((event) => <View key={event.event_id} style={styles.rawEvent}>
                <Text style={styles.newsMeta}>{event.event_type.toUpperCase()} · {new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</Text>
                <Text style={styles.newsHeadline}>{JSON.stringify(event)}</Text>
              </View>) : null}
            </View>
            <Text style={styles.disclaimer}>The plotted arrow is the persisted research decision vector. Feature inputs and attention links are shown above. This is not an order or a profitability claim.</Text>
            </> : <View style={styles.mechanicsSetView}>
              <MobileSetLedgerPanel ledger={setLedger} status={setLedgerStatus} />
              <MobileResearchLedger decision={vector} history={vectorHistory} events={rawEvents} />
              <View style={styles.rawTape}>
                <Pressable style={styles.rawTapeToggle} onPress={() => setShowRawEvents((value) => !value)}>
                  <Text style={styles.attentionTitle}>Persisted source-event tape · {rawEvents.length} observations</Text><Text style={styles.attentionCaption}>{showRawEvents ? "Hide" : "Show"}</Text>
                </Pressable>
                {showRawEvents ? rawEvents.slice(0,1000).map((event) => <View key={event.event_id} style={styles.rawEvent}>
                  <Text style={styles.newsMeta}>{event.event_type.toUpperCase()} · {new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</Text>
                  <Text style={styles.newsHeadline}>{JSON.stringify(event)}</Text>
                </View>) : null}
              </View>
            </View>}
          </ScrollView>
          <View style={styles.modalActions}><Pressable style={styles.closeAction} onPress={() => setSelected(null)}><Text style={styles.closeActionText}>Close</Text></Pressable><Pressable style={styles.saveAction} onPress={() => toggle(selected.symbol)}><Text style={styles.saveActionText}>{selected.archived ? "Remove from archive" : "Add to archive"}</Text></Pressable></View>
        </View></SafeAreaView> : null}
      </Modal>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  tracePanel: { gap: 10, padding: 13, borderRadius: 16, borderWidth: 0, borderColor: "transparent", backgroundColor: "#161F1A" },
  mechanicsTabs: { flexDirection: "row", gap: 6, padding: 6, marginBottom: 12, borderRadius: 13, borderWidth: 0, borderColor: "transparent", backgroundColor: "#161F1A" },
  mechanicsTab: { flex: 1, minHeight: 36, alignItems: "center", justifyContent: "center", borderRadius: 9 },
  mechanicsTabActive: { backgroundColor: "#243B28" },
  mechanicsTabText: { color: "#89968D", fontSize: 10, fontWeight: "700" },
  mechanicsTabTextActive: { color: "#7DF162" },
  mechanicsSetView: { gap: 13 },
  setOverlap: { flexDirection: "row", alignItems: "center", gap: 7, padding: 12, borderRadius: 11, borderWidth: 0, borderColor: "transparent", backgroundColor: "#0E1612" },
  setVenn: { alignItems: "center", gap: 5, padding: 10, borderRadius: 11, borderWidth: 0, borderColor: "transparent", backgroundColor: "#0E1612" },
  setVennUnion: { flexDirection: "row", alignItems: "center", gap: 8 },
  setOverlapSide: { flex: 1, alignItems: "center", gap: 4 },
  setOverlapMiddle: { flex: 1.1, alignItems: "center", gap: 4, paddingHorizontal: 6, borderLeftWidth: 1, borderRightWidth: 1, borderColor: "transparent" },
  setOverlapCount: { color: "#7DF162", fontSize: 18, fontWeight: "800" },
  vectorOutputBridge: { flexDirection: "row", gap: 10, padding: 12, borderRadius: 11, backgroundColor: "#17291F", borderWidth: 0, borderColor: "transparent" },
  mobileBacktest: { gap: 11, padding: 13, borderRadius: 15, borderWidth: 0, borderColor: "transparent", backgroundColor: "#161F1A" },
  backtestInputs: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  backtestInputWrap: { flexGrow: 1, flexBasis: "30%", minWidth: 85, gap: 5 },
  backtestInput: { borderWidth: 0, borderColor: "transparent", borderRadius: 9, paddingHorizontal: 9, paddingVertical: 7, color: "#EDF4EE", fontSize: 12 },
  backtestRun: { alignItems: "center", paddingVertical: 11, borderRadius: 22, backgroundColor: "#7DF162" },
  backtestDisabled: { opacity: 0.55 },
  backtestRunText: { color: "#161F1A", fontWeight: "800", fontSize: 11 },
  backtestMetrics: { flexDirection: "row", flexWrap: "wrap", gap: 7 },
  backtestMetric: { width: "48%", gap: 4, padding: 9, borderRadius: 9, backgroundColor: "#161F1A" },
  backtestFill: { paddingVertical: 8, borderBottomWidth: 0, borderBottomColor: "transparent", gap: 4 },
  traceSource: { gap: 4, padding: 10, borderRadius: 10, backgroundColor: "#161F1A" },
  traceValue: { color: "#EDF4EE", fontSize: 10, fontWeight: "700" },
  traceTableRow: { flexDirection: "row", borderBottomWidth: 0, borderBottomColor: "transparent" },
  traceCell: { width: 104, paddingHorizontal: 7, paddingVertical: 8, color: "#C5D0C7", fontSize: 8 },
  traceHead: { color: "#EDF4EE", fontWeight: "800", backgroundColor: "#161F1A" },
  traceGateGrid: { flexDirection: "row", flexWrap: "wrap", gap: 7 },
  traceGate: { width: "48%", gap: 4, padding: 9, borderRadius: 9, backgroundColor: "#161F1A" },
  traceSubhead: { color: "#EDF4EE", fontSize: 10, fontWeight: "800", marginTop: 5 },
  traceJson: { color: "#C5D0C7", backgroundColor: "#0E1612", padding: 9, borderRadius: 9, fontSize: 8, lineHeight: 12 },

  screen: { flex: 1, backgroundColor: "#0E1612" },
  topActions: { flexDirection: "row", alignItems: "center", gap: 10 },
  notificationButton: { width: 32, height: 32, borderRadius: 16, backgroundColor: "transparent", alignItems: "center", justifyContent: "center" },
  notificationGlyph: { color: "#7DF162", fontSize: 18 },
  notificationBadge: { position: "absolute", right: -5, top: -5, minWidth: 16, height: 16, paddingHorizontal: 3, borderRadius: 8, backgroundColor: "#7DF162", alignItems: "center", justifyContent: "center" },
  notificationBadgeText: { color: "#161F1A", fontSize: 7, fontWeight: "800" },
  notificationModal: { maxHeight: "88%", backgroundColor: "#161F1A", borderTopLeftRadius: 16, borderTopRightRadius: 16, overflow: "hidden" },
  notificationActions: { flexDirection: "row", justifyContent: "flex-end", gap: 18, paddingHorizontal: 18, paddingVertical: 10, borderBottomWidth: 0, borderBottomColor: "transparent" },
  notificationActionText: { color: "#7DF162", fontSize: 10, fontWeight: "700" },
  notificationContent: { padding: 14 },
  notificationItem: { paddingVertical: 12, borderBottomWidth: 0, borderBottomColor: "transparent" },
  notificationRead: { opacity: 0.6 },
  notificationMeta: { color: "#89968D", fontSize: 8, fontWeight: "800" },
  notificationTitle: { color: "#EDF4EE", fontSize: 11, fontWeight: "800", marginTop: 6 },
  notificationBody: { color: "#89968D", fontSize: 9, lineHeight: 14, marginTop: 4 },
  notificationPayload: { color: "#89968D", fontSize: 7, lineHeight: 11, marginTop: 7 },

  content: { paddingHorizontal: 17, paddingTop: 102, paddingBottom: 100 },
  topline: { position: "absolute", top: 0, left: 0, right: 0, zIndex: 12, height: 60, flexDirection: "row", alignItems: "center", justifyContent: "space-between", backgroundColor: "#0E1612" },
  marketStrip: { position: "absolute", top: 60, left: 0, right: 0, zIndex: 11, height: 42, justifyContent: "center", backgroundColor: "#0E1612" },
  marketStripContent: { paddingHorizontal: 17, alignItems: "center", gap: 8 },
  marketChip: { minHeight: 30, justifyContent: "center", paddingHorizontal: 6, paddingVertical: 4, borderBottomWidth: 1.5, borderBottomColor: "transparent", borderRadius: 0, backgroundColor: "transparent" },
  marketChipActive: { borderBottomColor: "#7DF162", backgroundColor: "transparent" },
  marketChipText: { color: "#89968D", fontSize: 10, fontWeight: "500" },
  marketChipTextActive: { color: "#7DF162" },
  brand: { color: "#7DF162", fontSize: 15, fontWeight: "700", letterSpacing: -0.3 },
  avatar: { width: 28, height: 28, borderRadius: 14, backgroundColor: "#17291F", alignItems: "center", justifyContent: "center" },
  avatarText: { color: "#7DF162", fontWeight: "800" },
  avatarImage: { width: 28, height: 28, borderRadius: 14 },
  accountShade: { flex: 1, justifyContent: "flex-start", alignItems: "flex-end", paddingTop: 58, paddingHorizontal: 16, backgroundColor: "#000000B8" },
  accountSheet: { width: 270, padding: 12, borderRadius: 12, backgroundColor: "#161F1A" },
  accountIdentity: { flexDirection: "row", alignItems: "center", gap: 10, padding: 9, marginBottom: 4 },
  accountAvatar: { width: 38, height: 38, borderRadius: 19, backgroundColor: "#17291F", alignItems: "center", justifyContent: "center", overflow: "hidden" },
  accountAvatarImage: { width: 38, height: 38, borderRadius: 19 },
  accountName: { color: "#EDF4EE", fontSize: 12, fontWeight: "800" },
  accountEmail: { color: "#89968D", fontSize: 9, marginTop: 3 },
  accountAction: { paddingVertical: 12, paddingHorizontal: 10, borderRadius: 11 },
  accountActionText: { color: "#C5D0C7", fontSize: 11, fontWeight: "600" },
  profileModal: { width: "100%", paddingBottom: 20, borderTopLeftRadius: 21, borderTopRightRadius: 21, backgroundColor: "#161F1A", overflow: "hidden" },
  profileIdentity: { flexDirection: "row", alignItems: "center", gap: 11, paddingHorizontal: 18, paddingVertical: 16 },
  profileAvatar: { width: 44, height: 44, borderRadius: 22, backgroundColor: "#17291F", alignItems: "center", justifyContent: "center", overflow: "hidden" },
  profileUidLabel: { color: "#89968D", fontSize: 8, fontWeight: "800", letterSpacing: 1, paddingHorizontal: 18, marginTop: 8 },
  profileUid: { color: "#C5D0C7", fontSize: 9, paddingHorizontal: 18, paddingTop: 6, paddingBottom: 15 },
  hero: { paddingTop: 24, paddingBottom: 25 },
  kicker: { color: "#7DF162", fontSize: 9, fontWeight: "800", letterSpacing: 1.5, marginBottom: 7 },
  heroTitle: { color: "#EDF4EE", fontSize: 42, lineHeight: 46, fontWeight: "800", letterSpacing: -1.7 },
  blue: { color: "#7DF162" },
  heroCopy: { color: "#89968D", fontSize: 12, lineHeight: 18, marginTop: 10, maxWidth: 300 },
  headingRow: { flexDirection: "row", alignItems: "flex-end", justifyContent: "space-between", marginBottom: 16 },
  heading: { color: "#EDF4EE", fontSize: 24, fontWeight: "800", letterSpacing: -.5 },
  subheading: { color: "#89968D", fontSize: 10, marginTop: 4 },
  count: { color: "#89968D", fontSize: 8, fontWeight: "800", letterSpacing: 1 },
  search: { height: 42, flexDirection: "row", alignItems: "center", borderWidth: 0, borderColor: "transparent", borderRadius: 8, backgroundColor: "#161F1A", paddingHorizontal: 12, marginBottom: 12 },
  searchIcon: { color: "#89968D", fontSize: 21, marginRight: 8 },
  searchInput: { flex: 1, color: "#EDF4EE", fontSize: 12, paddingVertical: 0 },
  filters: { flexDirection: "row", flexWrap: "wrap", gap: 7, marginBottom: 14 },
  filter: { borderWidth: 0, borderColor: "transparent", borderRadius: 18, paddingVertical: 6, paddingHorizontal: 11, backgroundColor: "#161F1A" },
  filterActive: { borderColor: "transparent", backgroundColor: "#17291F" },
  filterText: { color: "#89968D", fontSize: 10, fontWeight: "600" },
  filterTextActive: { color: "#7DF162" },
  coverageSection: { marginTop: 8, marginBottom: 18, paddingTop: 15, borderTopWidth: 0, borderTopColor: "transparent", borderBottomWidth: 0, borderBottomColor: "transparent", paddingBottom: 14 },
  coverageHeading: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 11 },
  coverageHeadingCopy: { flex: 1 },
  coverageSubheading: { color: "#89968D", fontSize: 9, lineHeight: 14, marginTop: 3 },
  coverageStatus: { color: "#7DF162", backgroundColor: "#17291F", borderRadius: 14, overflow: "hidden", paddingHorizontal: 8, paddingVertical: 5, fontSize: 8, fontWeight: "800" },
  coverageCard: { padding: 12, backgroundColor: "#161F1A", borderWidth: 0, borderColor: "transparent", borderRadius: 12, marginBottom: 8 },
  coverageCardTop: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  coverageName: { color: "#EDF4EE", fontSize: 12, fontWeight: "800" },
  coverageTag: { color: "#89968D", fontSize: 8, textTransform: "uppercase" },
  coveragePrimary: { color: "#7DF162", fontSize: 23, fontWeight: "800", marginVertical: 8 },
  coverageUnit: { color: "#89968D", fontSize: 9, fontWeight: "500" },
  coverageLine: { color: "#C5D0C7", fontSize: 9, lineHeight: 14, marginTop: 4 },
  coverageNote: { color: "#89968D", fontSize: 9, lineHeight: 14, marginTop: 8 },
  coverageDisclaimer: { color: "#89968D", fontSize: 9, lineHeight: 14, marginTop: 5 },
  card: { padding: 14, backgroundColor: "#161F1A", borderWidth: 0, borderColor: "transparent", borderRadius: 8, marginBottom: 9, shadowColor: "#000", shadowOpacity: 0, shadowRadius: 0, shadowOffset: { width: 0, height: 0 }, elevation: 0 },
  cardMain: { flexDirection: "row", alignItems: "center" },
  assetMark: { width: 37, height: 37, borderRadius: 12, alignItems: "center", justifyContent: "center" },
  assetMarkText: { color: "#161F1A", fontWeight: "800", fontSize: 15 },
  assetCopy: { flex: 1, marginLeft: 11 },
  assetName: { color: "#EDF4EE", fontSize: 13, fontWeight: "700" },
  assetMeta: { color: "#89968D", fontSize: 10, marginTop: 4 },
  arrow: { color: "#89968D", fontSize: 17 },
  detail: { color: "#89968D", fontSize: 10, marginTop: 12 },
  providerLine: { color: "#89968D", fontSize: 8, marginTop: 4 },
  cardLiveRow: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginTop: 8 },
  cardLivePrice: { color: "#EDF4EE", fontSize: 12, fontWeight: "700", fontVariant: ["tabular-nums"] },
  cardLiveDecision: { color: "#7DF162", fontSize: 8, fontWeight: "800" },
  addButton: { marginTop: 12, alignSelf: "flex-start", paddingVertical: 5 },
  addText: { color: "#7DF162", fontSize: 10, fontWeight: "700" },
  empty: { padding: 28, borderWidth: 0, borderStyle: "dashed", borderColor: "transparent", borderRadius: 14, alignItems: "center" },
  emptyTitle: { color: "#EDF4EE", fontSize: 13, fontWeight: "700" },
  emptyCopy: { color: "#89968D", fontSize: 10, textAlign: "center", marginTop: 6 },
  note: { marginTop: 15, padding: 14, borderWidth: 0, borderColor: "transparent", borderRadius: 12, backgroundColor: "#161F1A" },
  newsSection: { marginTop: 23, paddingTop: 18, borderTopWidth: 0, borderTopColor: "transparent" },
  macroSection: { marginTop: 20, paddingTop: 17, borderTopWidth: 0, borderTopColor: "transparent" },
  macroItem: { padding: 11, borderWidth: 0, borderColor: "transparent", borderRadius: 10, backgroundColor: "#161F1A", marginBottom: 7 },
  macroValues: { flexDirection: "row", flexWrap: "wrap", justifyContent: "space-between", gap: 8, marginTop: 7 },
  newsTitle: { color: "#EDF4EE", fontSize: 19, fontWeight: "800", marginBottom: 5 },
  newsItem: { padding: 12, backgroundColor: "#161F1A", borderWidth: 0, borderColor: "transparent", borderRadius: 11, marginBottom: 8 },
  newsMeta: { color: "#9AA6B5", fontSize: 8 },
  newsHeadline: { color: "#EDF4EE", fontSize: 12, fontWeight: "700", lineHeight: 17, marginTop: 7 },
  newsSummary: { color: "#89968D", fontSize: 10, lineHeight: 15, marginTop: 5 },
  noteTitle: { color: "#C5D0C7", fontSize: 11, fontWeight: "700" },
  noteCopy: { color: "#89968D", fontSize: 10, lineHeight: 15, marginTop: 5 },
  footer: { color: "#89968D", fontSize: 8, fontWeight: "700", letterSpacing: 1, textAlign: "center", marginTop: 25 },
  modalShade: { flex: 1, justifyContent: "flex-start", backgroundColor: "#161F1A" },
  modalCard: { flex: 1, width: "100%", height: "100%", maxHeight: "100%", backgroundColor: "#161F1A", borderRadius: 0, overflow: "hidden" },
  modalHeader: { flexDirection: "row", justifyContent: "space-between", alignItems: "flex-start", padding: 19, borderBottomWidth: 0, borderBottomColor: "transparent" },
  modalTitle: { color: "#EDF4EE", fontSize: 22, fontWeight: "800" },
  close: { width: 31, height: 31, borderRadius: 16, backgroundColor: "#17291F", alignItems: "center", justifyContent: "center" },
  closeText: { color: "#C5D0C7", fontSize: 20 },
  modalContent: { paddingHorizontal: 18, paddingTop: 15, paddingBottom: 18 },
  mapTitle: { color: "#C5D0C7", fontSize: 12, fontWeight: "700" },
  mapCopy: { color: "#89968D", fontSize: 10, marginTop: 4 },
  mapWrap: { width: "100%", marginTop: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 12, overflow: "hidden" },
  legend: { flexDirection: "row", flexWrap: "wrap", justifyContent: "center", gap: 12, marginVertical: 14 },
  legendItem: { flexDirection: "row", alignItems: "center", gap: 5 },
  legendDot: { width: 7, height: 7, borderRadius: 4 },
  legendText: { color: "#89968D", fontSize: 9 },
  metric: { flexDirection: "row", justifyContent: "space-between", paddingVertical: 9, borderBottomWidth: 0, borderBottomColor: "transparent" },
  metricLabel: { color: "#89968D", fontSize: 10, textTransform: "capitalize" },
  metricValue: { color: "#C5D0C7", fontSize: 10, fontWeight: "700" },
  engineMetrics: { flexDirection: "row", flexWrap: "wrap", gap: 9, marginVertical: 12 },
  engineLabel: { color: "#89968D", fontSize: 8, textTransform: "uppercase", letterSpacing: .5 },
  engineValue: { color: "#EDF4EE", fontSize: 12, fontWeight: "700", marginTop: 4 },
  attentionHint: { color: "#9AA6B5", fontSize: 8, marginTop: 5 },
  attentionPanel: { marginTop: 14, padding: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: "#161F1A" },
  livePanel: { marginTop: 12, padding: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: "#161F1A" },
  liveHeading: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginBottom: 9 },
  liveGrid: { flexDirection: "row", flexWrap: "wrap", gap: 10 },
  liveMetric: { width: "47%", gap: 4 },
  featureSpacePanel: { marginTop: 14, padding: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: "#161F1A" },
  strategyPanel: { marginTop: 14, padding: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: "#161F1A" },
  contributionPanel: { marginTop: 14, padding: 10, borderWidth: 0, borderColor: "transparent", borderRadius: 11, backgroundColor: "#161F1A" },
  contributionRow: { paddingVertical: 8, borderTopWidth: 0, borderTopColor: "transparent" },
  strategyGates: { flexDirection: "row", flexWrap: "wrap", gap: 10, marginVertical: 10 },
  strategyWeightRow: { paddingVertical: 8, borderTopWidth: 0, borderTopColor: "transparent" },
  strategyWeightValues: { flexDirection: "row", justifyContent: "space-between", flexWrap: "wrap", gap: 6, marginTop: 5 },
  rawTape: { marginTop: 14, borderWidth: 0, borderColor: "transparent", borderRadius: 11, overflow: "hidden" },
  rawTapeToggle: { padding: 11, flexDirection: "row", justifyContent: "space-between", alignItems: "center", backgroundColor: "#161F1A" },
  rawEvent: { padding: 9, borderTopWidth: 0, borderTopColor: "transparent" },
  featureValues: { flexDirection: "row", flexWrap: "wrap", justifyContent: "space-between", gap: 7 },
  featureValue: { width: "30%", flexDirection: "row", justifyContent: "space-between" },
  attentionHeader: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 9 },
  attentionTitle: { color: "#C5D0C7", fontSize: 10, fontWeight: "700" },
  attentionCaption: { color: "#9AA6B5", fontSize: 8 },
  attentionRow: { flexDirection: "row", alignItems: "center", gap: 3, marginBottom: 3 },
  attentionRowLabel: { width: 36, color: "#89968D", fontSize: 8, fontWeight: "700" },
  attentionAxis: { flex: 1, color: "#89968D", fontSize: 7, fontWeight: "700", textAlign: "center" },
  attentionCell: { flex: 1, minWidth: 0, height: 25, borderRadius: 4, alignItems: "center", justifyContent: "center" },
  attentionValue: { fontSize: 8, fontVariant: ["tabular-nums"] },
  disclaimer: { color: "#89968D", fontSize: 9, lineHeight: 14, padding: 11, backgroundColor: "#0E1612", borderRadius: 9, marginTop: 15 },
  modalActions: { flexDirection: "row", justifyContent: "flex-end", gap: 8, padding: 14, borderTopWidth: 0, borderTopColor: "transparent" },
  closeAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#243B28" },
  closeActionText: { color: "#89968D", fontSize: 11, fontWeight: "700" },
  saveAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#7DF162" },
  saveActionText: { color: "#161F1A", fontSize: 11, fontWeight: "700" },

  cardMetaRow: { flexDirection: "row", flexWrap: "wrap", gap: 5, marginLeft: 48, marginTop: 9, marginBottom: 9 },
  cardMetaChip: { overflow: "hidden", color: "#89968D", backgroundColor: "#161F1A", borderWidth: 0, borderColor: "transparent", borderRadius: 12, paddingHorizontal: 7, paddingVertical: 4, fontSize: 7, fontWeight: "800", letterSpacing: 0.3 },
  cardMetaLive: { color: "#7DF162", borderColor: "transparent", backgroundColor: "#243B28" },
  cardPager: { flexDirection: "row", minHeight: 90, gap: 10, padding: 10, borderRadius: 13, borderWidth: 0, borderColor: "transparent", backgroundColor: "#161F1A" },
  cardPagerPrimary: { flex: 1, justifyContent: "center", gap: 5, minWidth: 0 },
  cardKicker: { color: "#89968D", fontSize: 7, fontWeight: "800", letterSpacing: 1 },
  cardPagerValue: { color: "#EDF4EE", fontSize: 18, fontWeight: "800", fontVariant: ["tabular-nums"] },
  cardPagerStatus: { color: "#7DF162", fontSize: 7, fontWeight: "900", letterSpacing: 0.8 },
  cardPagerNegative: { color: "#E47A82" },
  cardPagerSecondary: { width: "39%", justifyContent: "center", gap: 4, paddingLeft: 9, borderLeftWidth: 0, borderLeftColor: "transparent" },
  cardMetaLabel: { color: "#89968D", fontSize: 7, fontWeight: "800" },
  cardMetaValue: { color: "#C5D0C7", fontSize: 9, fontWeight: "700" },
  cardPagerReason: { flex: 1, justifyContent: "center", gap: 5, minWidth: 0, paddingLeft: 9, borderLeftWidth: 0, borderLeftColor: "transparent" },
  cardReasonText: { color: "#C5D0C7", fontSize: 8, lineHeight: 12 },
  cardPagerFooter: { flexDirection: "row", alignItems: "center", gap: 8, marginLeft: 48, marginTop: 8 },
  cardPagerDots: { flexDirection: "row", alignItems: "center", gap: 5 },
  cardPagerDot: { width: 6, height: 6, borderRadius: 4, backgroundColor: "#365247" },
  cardPagerDotActive: { width: 18, backgroundColor: "#7DF162" },
  cardSwipeHint: { color: "#687A92", fontSize: 7, fontWeight: "800", letterSpacing: 1 },

});
