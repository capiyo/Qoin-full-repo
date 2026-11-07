import React, { useEffect, useMemo, useState } from "react";
import { ScrollView, View, Text, TextInput, Pressable, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import {
  evaluateQoinSignal,
  getQoinHealth,
  getOperatingCycle,
  configureOperatingCycle,
  startOperatingCycle,
  stopOperatingCycle,
  runPaperBacktest,
  getPaperBacktestHistory,
  getPaperSession,
  getPaperSessionHistory,
  type PaperBacktestReport,
  type PaperBacktestHistoryItem,
  type MarketForceObservation,
  type QoinSignal,
  type OperatingCycleConfig,
  type OperatingCycleSnapshot,
  type PaperAccountConfig,
  type PaperSessionSnapshot,
  type PaperMirrorEvent,
  type PaperTradeHistoryItem,
} from "@qoin/core";
import { colors } from "../theme";

const initial: MarketForceObservation = {
  ofi: 1.2,
  liquidity: 0.4,
  volatility: 0.3,
  funding: -0.2,
  open_interest_change: 0.6,
  liquidation_pressure: 0.1,
  basis: 0,
};

const fields: Array<{ key: keyof MarketForceObservation; label: string; help: string }> = [
  { key: "ofi", label: "Order-flow imbalance", help: "Aggressive buy/sell pressure" },
  { key: "liquidity", label: "Liquidity / depth", help: "Normalized liquidity change" },
  { key: "volatility", label: "Volatility", help: "Context and directional dampening" },
  { key: "funding", label: "Funding", help: "Perpetual positioning pressure" },
  { key: "open_interest_change", label: "Open interest change", help: "Change in open positions" },
  { key: "liquidation_pressure", label: "Liquidation pressure", help: "Forced-flow intensity" },
  { key: "basis", label: "Basis", help: "Perpetual / spot dislocation" },
];

export function SignalScreen() {
  const [forces, setForces] = useState(initial);
  const [signal, setSignal] = useState<QoinSignal | null>(null);
  const [apiOnline, setApiOnline] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [cycle, setCycle] = useState<OperatingCycleSnapshot | null>(null);
  const [cycleConfig, setCycleConfig] = useState<OperatingCycleConfig>({ observation_minutes: 5, weight_evaluation_minutes: 60, session_minutes: 480, mode: "paper", paper_accounts: [] });
  const [cycleBusy, setCycleBusy] = useState(false);
  const [cycleError, setCycleError] = useState("");
  const [paperSession, setPaperSession] = useState<PaperSessionSnapshot | null>(null);
  const [paperHistory, setPaperHistory] = useState<{ decisions: PaperMirrorEvent[]; trades: PaperTradeHistoryItem[] }>({ decisions: [], trades: [] });
  const [paperAccountDraft, setPaperAccountDraft] = useState<PaperAccountConfig>({ symbol: "", venue: "", quote_currency: "USD", initial_cash: 0, notional_per_trade: 0, contract_multiplier: 1, quantity_step: 0.000001, reinvest_profits: true, fee_bps: 10, slippage_bps: 5 });
  const [backtest, setBacktest] = useState<PaperBacktestReport | null>(null);
  const [backtestHistory, setBacktestHistory] = useState<PaperBacktestHistoryItem[]>([]);
  const [backtestBusy, setBacktestBusy] = useState(false);
  const [backtestError, setBacktestError] = useState("");
  const [backtestForm, setBacktestForm] = useState({ symbol: "BTCUSDT", venue: "", quote_currency: "USDT", lookback_hours: "24", warmup_hours: "24", initial_cash: "0", notional_per_trade: "0", reinvest_profits: true, fee_bps: "10", slippage_bps: "5" });

  useEffect(() => {
    getPaperBacktestHistory().then((result) => setBacktestHistory(result.runs)).catch(() => setBacktestHistory([]));
    getQoinHealth().then((health) => setApiOnline(health.status === "ok" && !health.live_trading))
      .catch(() => setApiOnline(false));
    getOperatingCycle().then((snapshot) => { setCycle(snapshot); setCycleConfig(snapshot.config); }).catch(() => setCycle(null));
    getPaperSession().then(setPaperSession).catch(() => setPaperSession(null));
    getPaperSessionHistory(undefined, 50).then(setPaperHistory).catch(() => setPaperHistory({ decisions: [], trades: [] }));
    const refreshCycle = () => { getOperatingCycle().then(setCycle).catch(() => setCycle(null)); getPaperSession().then(setPaperSession).catch(() => setPaperSession(null)); getPaperSessionHistory(undefined, 50).then(setPaperHistory).catch(() => undefined); };
    const timer = setInterval(refreshCycle, 5000);
    return () => clearInterval(timer);
  }, []);

  const activeCount = useMemo(
    () => fields.filter(({ key }) => Math.abs(forces[key]) > 0.0001).length,
    [forces],
  );

  async function saveCycle() {
    setCycleBusy(true); setCycleError("");
    try { setCycle(await configureOperatingCycle(cycleConfig)); }
    catch (err) { setCycleError(err instanceof Error ? err.message : "Could not save cycle configuration"); }
    finally { setCycleBusy(false); }
  }

  async function runCycle(action: "start" | "stop") {
    setCycleBusy(true); setCycleError("");
    try {
      if (action === "start") { await configureOperatingCycle(cycleConfig); setCycle(await startOperatingCycle()); }
      else setCycle(await stopOperatingCycle());
      setPaperSession(await getPaperSession());
      setPaperHistory(await getPaperSessionHistory(undefined, 50));
    }
    catch (err) { setCycleError(err instanceof Error ? err.message : "Could not update operating cycle"); }
    finally { setCycleBusy(false); }
  }

  async function runBacktest() {
    setBacktestBusy(true); setBacktestError(""); setBacktest(null);
    const end = Date.now();
    try {
      const result = await runPaperBacktest({ symbol: backtestForm.symbol.trim().toUpperCase(), venue: backtestForm.venue.trim(), quote_currency: backtestForm.quote_currency.trim().toUpperCase(), start_ts_ms: end - Number(backtestForm.lookback_hours) * 3600000, end_ts_ms: end, warmup_hours: Number(backtestForm.warmup_hours), initial_cash: Number(backtestForm.initial_cash), notional_per_trade: Number(backtestForm.notional_per_trade), reinvest_profits: backtestForm.reinvest_profits, fee_bps: Number(backtestForm.fee_bps), slippage_bps: Number(backtestForm.slippage_bps), max_events: 100000 });
      setBacktest(result);
      const history = await getPaperBacktestHistory(backtestForm.symbol.trim().toUpperCase());
      setBacktestHistory(history.runs);
    } catch (err) { setBacktestError(err instanceof Error ? err.message : "Paper replay failed"); }
    finally { setBacktestBusy(false); }
  }

  function addPaperAccount() {
    const account = { ...paperAccountDraft, symbol: paperAccountDraft.symbol.trim().toUpperCase(), venue: paperAccountDraft.venue.trim(), quote_currency: paperAccountDraft.quote_currency.trim().toUpperCase() };
    if (!account.symbol || !account.venue || !account.quote_currency || account.initial_cash <= 0 || account.notional_per_trade <= 0 || account.notional_per_trade > account.initial_cash || account.contract_multiplier <= 0 || account.quantity_step <= 0) { setCycleError("Enter symbol, exact venue, quote currency, positive capital and a notional within that capital."); return; }
    if (cycleConfig.paper_accounts.some((item) => item.symbol === account.symbol && item.venue === account.venue)) { setCycleError("This venue and symbol are already configured."); return; }
    if (cycleConfig.paper_accounts.length >= 50) { setCycleError("A cycle supports at most 50 instruments."); return; }
    setCycleConfig((current) => ({ ...current, paper_accounts: [...current.paper_accounts, account] })); setCycleError("");
  }
  function removePaperAccount(symbol: string, venue: string) {
    setCycleConfig((current) => ({ ...current, paper_accounts: current.paper_accounts.filter((item) => item.symbol !== symbol || item.venue !== venue) }));
  }

  async function evaluate() {
    setBusy(true);
    setError("");
    try {
      const result = await evaluateQoinSignal({ observation: forces, has_open_position: false });
      setSignal(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reach the Qoin API.");
    } finally {
      setBusy(false);
    }
  }

  const decisionColor = signal?.decision === "BUY" ? colors.bullish
    : signal?.decision === "SELL" ? colors.bearish : colors.accentCyan;

  return (
    <SafeAreaView style={styles.screen}>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        <View style={styles.header}>
          <View>
            <Text style={styles.kicker}>QOIN / RESEARCH ENGINE</Text>
            <Text style={styles.title}>Force vectors</Text>
          </View>
          <View style={styles.statusPill}>
            <View style={[styles.dot, { backgroundColor: apiOnline ? colors.bullish : colors.textMuted }]} />
            <Text style={styles.statusText}>{apiOnline ? "API online" : "API offline"}</Text>
          </View>
        </View>
        <Text style={styles.subtitle}>Explore current normalized forces and evaluate a versioned signal. This screen never submits orders.</Text>

        <View style={styles.panel}>
          <View style={styles.panelHeader}>
            <Text style={styles.panelTitle}>Operating cycle</Text>
            <Text style={styles.counter}>{(cycle?.phase ?? "loading").replace(/_/g, " ").toUpperCase()}</Text>
          </View>
          <Text style={styles.fieldHelp}>5-minute observation → 60-minute feature/attention diagnostics → paper monitoring. Coefficients stay fixed; no online self-training.</Text>
          {([
            ["observation_minutes", "Observation (min)"],
            ["weight_evaluation_minutes", "Weight evaluation (min)"],
            ["session_minutes", "Paper session (min)"],
          ] as const).map(([key, label]) => (
            <View key={key} style={styles.fieldRow}>
              <Text style={styles.fieldLabel}>{label}</Text>
              <TextInput accessibilityLabel={label} keyboardType="number-pad" value={String(cycleConfig[key])}
                editable={!cycle?.active}
                onChangeText={(value) => setCycleConfig((current) => ({ ...current, [key]: Math.max(1, Number(value) || 1) }))}
                style={styles.input} />
            </View>
          ))}
          <Text style={styles.panelTitle}>Paper accounts · {cycleConfig.paper_accounts.length}/50</Text>
          <Text style={styles.fieldHelp}>Each venue/symbol uses an isolated quote-currency ledger. Set the contract multiplier and lot step from the venue specification; no FX conversion.</Text>
          {Object.entries(cycleConfig.paper_accounts.reduce<Record<string, number>>((totals, account) => { totals[account.quote_currency] = (totals[account.quote_currency] ?? 0) + account.initial_cash; return totals; }, {})).map(([currency, total]) => <Text key={currency} style={styles.fieldHelp}>Allocated {currency}: {total.toLocaleString()}</Text>)}
          <TextInput accessibilityLabel="Paper symbol" placeholder="Symbol e.g. BTCUSDT" value={paperAccountDraft.symbol} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, symbol: value }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Exact venue" placeholder="Exact ingestion venue ID" value={paperAccountDraft.venue} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, venue: value }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Quote currency" placeholder="Quote currency e.g. USDT" value={paperAccountDraft.quote_currency} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, quote_currency: value }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Starting capital" placeholder="Starting capital in quote currency" keyboardType="decimal-pad" value={String(paperAccountDraft.initial_cash || "")} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, initial_cash: Number(value) || 0 }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Notional per entry" placeholder="Notional per entry" keyboardType="decimal-pad" value={String(paperAccountDraft.notional_per_trade || "")} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, notional_per_trade: Number(value) || 0 }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Contract multiplier" placeholder="Contract multiplier from venue spec" keyboardType="decimal-pad" value={String(paperAccountDraft.contract_multiplier)} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, contract_multiplier: Number(value) || 0 }))} style={styles.input} editable={!cycle?.active} />
          <TextInput accessibilityLabel="Quantity step" placeholder="Minimum quantity increment / lot step" keyboardType="decimal-pad" value={String(paperAccountDraft.quantity_step)} onChangeText={(value) => setPaperAccountDraft((v) => ({ ...v, quantity_step: Number(value) || 0 }))} style={styles.input} editable={!cycle?.active} />
          <Pressable onPress={() => setPaperAccountDraft((v) => ({ ...v, reinvest_profits: !v.reinvest_profits }))} style={styles.secondaryButton}><Text style={styles.secondaryButtonText}>Compound future entries: {paperAccountDraft.reinvest_profits ? "ON" : "OFF"}</Text></Pressable>
          <View style={styles.metricRow}><Text style={styles.metricLabel}>Fee / slippage (bps per side)</Text><Text style={styles.metricValue}>{paperAccountDraft.fee_bps} / {paperAccountDraft.slippage_bps}</Text></View>
          <View style={styles.cycleButtons}>
            <Pressable onPress={() => setPaperAccountDraft((v) => ({ ...v, fee_bps: Math.max(0, v.fee_bps - 1) }))} style={styles.secondaryButton}><Text style={styles.secondaryButtonText}>Fee −</Text></Pressable>
            <Pressable onPress={() => setPaperAccountDraft((v) => ({ ...v, fee_bps: Math.min(100, v.fee_bps + 1) }))} style={styles.secondaryButton}><Text style={styles.secondaryButtonText}>Fee +</Text></Pressable>
            <Pressable onPress={() => setPaperAccountDraft((v) => ({ ...v, slippage_bps: Math.max(0, v.slippage_bps - 1) }))} style={styles.secondaryButton}><Text style={styles.secondaryButtonText}>Slip −</Text></Pressable>
            <Pressable onPress={() => setPaperAccountDraft((v) => ({ ...v, slippage_bps: Math.min(100, v.slippage_bps + 1) }))} style={styles.secondaryButton}><Text style={styles.secondaryButtonText}>Slip +</Text></Pressable>
          </View>
          <Pressable onPress={addPaperAccount} disabled={!!cycle?.active} style={[styles.secondaryButton, !!cycle?.active && styles.disabled]}><Text style={styles.secondaryButtonText}>Add instrument account</Text></Pressable>
          {cycleConfig.paper_accounts.map((account) => (
            <View key={account.venue + account.symbol} style={styles.metricRow}>
              <Text style={styles.metricLabel}>{account.symbol} · {account.venue} · {account.quote_currency} {account.initial_cash} · x{account.contract_multiplier} · step {account.quantity_step}</Text>
              <Pressable onPress={() => removePaperAccount(account.symbol, account.venue)} disabled={!!cycle?.active}><Text style={styles.secondaryButtonText}>Remove</Text></Pressable>
            </View>
          ))}
          <View style={styles.metricRow}><Text style={styles.metricLabel}>Seconds remaining</Text><Text style={styles.metricValue}>{cycle?.phase_seconds_remaining ?? 0}</Text></View>
          <View style={styles.metricRow}><Text style={styles.metricLabel}>Observations / evaluations</Text><Text style={styles.metricValue}>{cycle?.observations ?? 0} / {cycle?.signal_evaluations ?? 0}</Text></View>
          <View style={styles.metricRow}><Text style={styles.metricLabel}>Mean |score| / alignment</Text><Text style={styles.metricValue}>{cycle?.mean_absolute_score?.toFixed(3) ?? "—"} / {cycle?.mean_alignment?.toFixed(3) ?? "—"}</Text></View>
          <View style={styles.metricRow}><Text style={styles.metricLabel}>BUY / SELL / WAIT</Text><Text style={styles.metricValue}>{cycle?.buy_signals ?? 0} / {cycle?.sell_signals ?? 0} / {cycle?.wait_signals ?? 0}</Text></View>
          {cycle && Object.keys(cycle.mean_feature_values).length > 0 ? <>
            <Text style={styles.panelTitle}>Mean feature activations</Text>
            {Object.entries(cycle.mean_feature_values).map(([name, value]) => (
              <View key={name} style={styles.metricRow}><Text style={styles.metricLabel}>{name}</Text><Text style={styles.metricValue}>{value.toFixed(3)}</Text></View>
            ))}
          </> : null}
          {cycle && Object.keys(cycle.mean_weight_contributions).length > 0 ? <>
            <Text style={styles.panelTitle}>Mean weighted contribution · bullish / bearish</Text>
            {Object.entries(cycle.mean_weight_contributions).map(([name, values]) => <View key={name} style={styles.metricRow}><Text style={styles.metricLabel}>{name}</Text><Text style={styles.metricValue}>{values[0].toFixed(3)} / {values[1].toFixed(3)}</Text></View>)}
          </> : null}
          {cycle && cycle.mean_attention.length > 0 ? <>
            <Text style={styles.panelTitle}>Mean attention matrix</Text>
            {cycle.mean_attention.map((values, index) => (
              <Text key={index} style={styles.fieldHelp}>Row {index + 1}: {values.map((value) => value.toFixed(3)).join(" · ")}</Text>
            ))}
          </> : null}
          <View style={styles.cycleButtons}>
            <Pressable onPress={saveCycle} disabled={cycleBusy || !!cycle?.active} style={[styles.secondaryButton, (cycleBusy || !!cycle?.active) && styles.disabled]}><Text style={styles.secondaryButtonText}>Save</Text></Pressable>
            <Pressable onPress={() => runCycle("start")} disabled={cycleBusy || !!cycle?.active || (cycleConfig.mode === "paper" && cycleConfig.paper_accounts.length === 0)} style={[styles.cycleButton, (cycleBusy || !!cycle?.active || (cycleConfig.mode === "paper" && cycleConfig.paper_accounts.length === 0)) && styles.disabled]}><Text style={styles.buttonText}>Start</Text></Pressable>
            <Pressable onPress={() => runCycle("stop")} disabled={cycleBusy || !cycle?.active} style={[styles.secondaryButton, (cycleBusy || !cycle?.active) && styles.disabled]}><Text style={styles.secondaryButtonText}>Stop</Text></Pressable>
          </View>
          {cycleError ? <Text accessibilityRole="alert" style={styles.error}>{cycleError}</Text> : null}
          <Text style={styles.cycleSafety}>PAPER SIMULATION ONLY · NO BROKER ORDERS · LIVE EXECUTION DISABLED</Text>
        </View>
        <View style={styles.panel}>
          <View style={styles.panelHeader}>
            <Text style={styles.panelTitle}>Mirror paper backtest</Text>
            <Text style={styles.counter}>NO LIVE ORDERS</Text>
          </View>
          <Text style={styles.fieldHelp}>Chronological replay of persisted MongoDB events using the active strategy. Simulated buys use ask; sells use bid, plus fees and slippage. Long-only. Capital/notional are in quote currency: convert KSh 20,000 to USDT first for BTCUSDT. No FX conversion is applied.</Text>
          {([
            ["symbol", "Symbol"],
            ["venue", "Exact venue ID"],
            ["quote_currency", "Quote currency (capital unit)"],
            ["lookback_hours", "Lookback hours"],
            ["warmup_hours", "Warmup history hours"],
            ["initial_cash", "Starting capital"],
            ["notional_per_trade", "Notional per entry"],
            ["fee_bps", "Fee bps / side"],
            ["slippage_bps", "Slippage bps"],
          ] as const).map(([key, label]) => (
            <View key={key} style={styles.fieldRow}>
              <Text style={styles.fieldLabel}>{label}</Text>
              <TextInput accessibilityLabel={label} value={backtestForm[key]}
                keyboardType={key === "symbol" ? "default" : "decimal-pad"}
                autoCapitalize={key === "symbol" ? "characters" : "none"}
                onChangeText={(value) => setBacktestForm((current) => ({ ...current, [key]: value }))}
                style={styles.input} />
            </View>
          ))}
          <Pressable onPress={() => setBacktestForm((current) => ({ ...current, reinvest_profits: !current.reinvest_profits }))} style={styles.metricRow}>
            <Text style={styles.metricLabel}>Reinvest profits / compound sizing</Text>
            <Text style={styles.metricValue}>{backtestForm.reinvest_profits ? "ON" : "OFF"}</Text>
          </Pressable>
          <Pressable onPress={runBacktest} disabled={backtestBusy || !backtestForm.symbol.trim() || !backtestForm.venue.trim() || !backtestForm.quote_currency.trim() || Number(backtestForm.initial_cash) <= 0 || Number(backtestForm.notional_per_trade) <= 0 || Number(backtestForm.notional_per_trade) > Number(backtestForm.initial_cash) || Number(backtestForm.lookback_hours) <= 0}
            style={[styles.cycleButton, backtestBusy && styles.disabled]}>
            <Text style={styles.buttonText}>{backtestBusy ? "Replaying event history…" : "Run chronological paper replay"}</Text>
          </Pressable>
          {backtestError ? <Text accessibilityRole="alert" style={styles.error}>{backtestError}</Text> : null}
          {backtest ? <>
            {([
              ["Net P&L", backtest.net_pnl.toFixed(2)],
              ["Return", backtest.return_pct.toFixed(2) + "%"],
              ["Max drawdown", backtest.max_drawdown_pct.toFixed(2) + "%"],
              ["Win rate", backtest.win_rate_pct == null ? "—" : backtest.win_rate_pct.toFixed(1) + "%"],
              ["Closed trades", String(backtest.executed_exits)],
              ["Fees paid", backtest.fees_paid.toFixed(2)],
              ["Events / signals", backtest.event_count + " / " + backtest.signal_count],
              ["Warmup events", String(backtest.warmup_event_count)],
              ["Skipped entries", String(backtest.skipped_entries)],
              ["Pending intent", backtest.pending_order ?? "—"],
            ] as const).map(([label, value]) => (
              <View key={label} style={styles.metricRow}><Text style={styles.metricLabel}>{label}</Text><Text style={styles.metricValue}>{value}</Text></View>
            ))}
            <Text style={styles.fieldHelp}>Run {backtest.run_id} · {backtest.venue} · {backtest.quote_currency} · compounding {backtest.reinvest_profits ? "ON" : "OFF"} · {backtest.decisions.length} Mirror decisions persisted · {backtest.strategy_version}</Text>
            <Text style={styles.fieldHelp}>{backtest.assumptions}</Text>
            {backtestHistory.length > 0 ? <>
              <Text style={styles.panelTitle}>Saved Mirror runs</Text>
              {backtestHistory.slice(0, 6).map((run) => (
                <View key={run.run_id} style={styles.metricRow}>
                  <View style={{ flex: 1 }}>
                    <Text style={styles.metricLabel}>{run.symbol} · {run.run_id}</Text>
                    <Text style={styles.fieldHelp}>{run.return_pct.toFixed(2)}% · drawdown {run.max_drawdown_pct.toFixed(2)}%</Text>
                  </View>
                  <Text style={styles.metricValue}>{run.net_pnl.toFixed(2)}</Text>
                </View>
              ))}
            </> : null}
            <Text style={styles.panelTitle}>Mirror trace · latest decisions</Text>
            {backtest.decisions.slice(-8).reverse().map((item) => (
              <View key={item.sequence + "-" + item.event_id} style={styles.mirrorEntry}>
                <View style={styles.metricRow}>
                  <Text style={styles.metricLabel}>{item.action.toUpperCase()} · {item.execution.replace(/_/g, " ")}</Text>
                  <Text style={styles.metricValue}>{item.magnitude.toFixed(3)}</Text>
                </View>
                <Text style={styles.fieldHelp}>{item.reason}</Text>
                <Text style={styles.fieldHelp}>Vector [{item.decision_vector.map((value) => value.toFixed(2)).join(", ")}] · forces {Object.entries(item.features).map(([name, value]) => name + " " + value.toFixed(2)).join(" / ")}</Text>
              </View>
            ))}
            <Text style={styles.panelTitle}>Closed trade ledger · latest 5</Text>
            {backtest.trades.slice(-5).reverse().map((trade, index) => (
              <View key={trade.entry_ts_ms + "-" + index} style={styles.metricRow}>
                <Text style={styles.metricLabel}>Net {trade.net_pnl?.toFixed(2) ?? "—"} · {trade.return_pct?.toFixed(2) ?? "—"}%</Text>
                <Text style={styles.metricValue}>{trade.exit_reason ?? "open"}</Text>
              </View>
            ))}
            {backtest.event_count === 0 ? <Text style={styles.error}>No events found. Ingest historical market data before interpreting this result.</Text> : null}
          </> : null}
        </View>
                        <View style={styles.panel}>
          <View style={styles.panelHeader}><Text style={styles.panelTitle}>Paper account ledger</Text><Text style={styles.counter}>{cycle?.phase === "paper_session" ? "PAPER ACTIVE" : cycle?.active && cycle.config.mode === "paper" ? "ARMED · WAITING" : "INACTIVE"}</Text></View>
          {paperSession?.accounts.length ? paperSession.accounts.map((account) => (
            <View key={account.venue + account.symbol} style={styles.metricRow}>
              <View style={{ flex: 1 }}>
                <Text style={styles.fieldLabel}>{account.symbol} · {account.venue}</Text>
                <Text style={styles.fieldHelp}>{account.quote_currency} · Equity {account.equity.toFixed(2)} · Net P/L {account.net_pnl.toFixed(2)}</Text>
                <Text style={styles.fieldHelp}>Realized {account.realized_pnl.toFixed(2)} · Unrealized {account.unrealized_pnl.toFixed(2)} · Fees {account.fees_paid.toFixed(2)} · Max DD {account.max_drawdown_pct.toFixed(2)}%</Text>
                <Text style={styles.fieldHelp}>Entries / exits {account.entries} / {account.exits} · Events {account.events} · {account.has_open_position ? `POSITION OPEN · ${account.open_position?.quantity.toFixed(6)} @ ${account.open_position?.entry_price.toFixed(5)}` : "FLAT"}</Text>
              </View>
            </View>
          )) : <Text style={styles.fieldHelp}>No paper session activity yet. Add accounts and start a cycle.</Text>}
          <Text style={styles.cycleSafety}>SIMULATED FILLS ONLY · LIVE BROKER ROUTING DISABLED</Text>
          <Text style={styles.panelTitle}>Recent Mirror decisions</Text>
          {paperHistory.decisions.slice(0, 8).map((item) => <View key={item.venue + item.symbol + item.event_id} style={styles.metricRow}><View style={{ flex: 1 }}><Text style={styles.fieldLabel}>{item.symbol} · {item.venue} · {item.action.toUpperCase()}</Text><Text style={styles.fieldHelp}>{item.execution} · {item.reason}</Text><Text style={styles.fieldHelp}>Equity {item.equity.toFixed(2)} {item.quote_currency} · {new Date(item.ts_ms).toLocaleTimeString()}</Text></View></View>)}
          {!paperHistory.decisions.length ? <Text style={styles.fieldHelp}>No paper Mirror events yet.</Text> : null}
          <Text style={styles.panelTitle}>Closed paper trades</Text>
          {paperHistory.trades.slice(0, 8).map((trade, index) => <View key={trade.cycle_id + trade.venue + trade.symbol + trade.exit_ts_ms + index} style={styles.metricRow}><View style={{ flex: 1 }}><Text style={styles.fieldLabel}>{trade.symbol} · {trade.venue}</Text><Text style={styles.fieldHelp}>Net P/L {trade.net_pnl.toFixed(2)} {trade.quote_currency} · {trade.return_pct.toFixed(3)}%</Text><Text style={styles.fieldHelp}>Fees {trade.fees.toFixed(2)} · {new Date(trade.exit_ts_ms).toLocaleString()}</Text></View></View>)}
          {!paperHistory.trades.length ? <Text style={styles.fieldHelp}>No closed paper trades yet.</Text> : null}
        </View>
<View style={styles.panel}>
          <View style={styles.panelHeader}>
            <Text style={styles.panelTitle}>Observation vector</Text>
            <Text style={styles.counter}>{activeCount}/7 active</Text>
          </View>
          {fields.map(({ key, label, help }) => (
            <View key={key} style={styles.fieldRow}>
              <View style={styles.fieldCopy}>
                <Text style={styles.fieldLabel}>{label}</Text>
                <Text style={styles.fieldHelp}>{help}</Text>
              </View>
              <TextInput
                accessibilityLabel={label}
                keyboardType="decimal-pad"
                value={String(forces[key])}
                onChangeText={(value) => {
                  const parsed = Number(value);
                  if (value.trim() !== "" && Number.isFinite(parsed)) {
                    setForces((current) => ({ ...current, [key]: parsed }));
                  } else if (value.trim() === "" || value === "-") {
                    setForces((current) => ({ ...current, [key]: 0 }));
                  }
                }}
                style={styles.input}
                selectTextOnFocus
              />
            </View>
          ))}
          <Pressable onPress={evaluate} disabled={busy} style={[styles.button, busy && styles.disabled]}>
            <Text style={styles.buttonText}>{busy ? "Evaluating…" : "Evaluate research signal"}</Text>
          </Pressable>
          {error ? <Text accessibilityRole="alert" style={styles.error}>{error}</Text> : null}
        </View>

        <View style={styles.panel}>
          <Text style={styles.kicker}>VECTOR CONVERGENCE</Text>
          <Text style={styles.panelTitle}>Signal output</Text>
          {signal ? (
            <>
              <Text style={[styles.decision, { color: decisionColor }]}>{signal.decision}</Text>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Directional score</Text><Text style={styles.metricValue}>{signal.score.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Angular alignment</Text><Text style={styles.metricValue}>{signal.alignment.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Angle (radians)</Text><Text style={styles.metricValue}>{signal.angle_radians.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Active forces</Text><Text style={styles.metricValue}>{signal.active_force_count}/7</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Config version</Text><Text style={styles.metricValue}>{signal.config_version}</Text></View>
              <Text style={styles.rationale}>{signal.rationale}</Text>
            </>
          ) : (
            <Text style={styles.empty}>No signal evaluated yet. Inputs are manually supplied normalized observations, not a live exchange feed.</Text>
          )}
        </View>
        <View style={styles.safety}>
          <Text style={styles.safetyTitle}>Research only · no live execution</Text>
          <Text style={styles.safetyBody}>The baseline is not calibrated and has no demonstrated profitability. Signals do not size positions, pass a risk gate, or place orders.</Text>
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary },
  content: { padding: 16, paddingBottom: 28 },
  header: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", gap: 12, marginBottom: 12 },
  kicker: { color: colors.accentCyan, fontSize: 10, fontWeight: "800", letterSpacing: 1.5, marginBottom: 6 },
  title: { color: colors.white, fontSize: 25, fontWeight: "800" },
  subtitle: { color: colors.textMuted, fontSize: 13, lineHeight: 19, marginBottom: 16 },
  statusPill: { flexDirection: "row", alignItems: "center", gap: 6, paddingHorizontal: 9, paddingVertical: 7, borderRadius: 20, backgroundColor: colors.bgSecondary },
  dot: { width: 6, height: 6, borderRadius: 3 },
  statusText: { color: colors.textMuted, fontSize: 10, fontWeight: "600" },
  panel: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 14, padding: 16, marginBottom: 14 },
  panelHeader: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginBottom: 6 },
  panelTitle: { color: colors.white, fontSize: 18, fontWeight: "700", marginBottom: 12 },
  counter: { color: colors.accentCyan, fontSize: 11, fontWeight: "700" },
  fieldRow: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 10, paddingVertical: 11, borderBottomWidth: 1, borderBottomColor: colors.border },
  fieldCopy: { flex: 1 },
  fieldLabel: { color: colors.textPrimary, fontSize: 12, fontWeight: "700" },
  fieldHelp: { color: colors.textMuted, fontSize: 10, marginTop: 3 },
  input: { width: 92, color: colors.white, backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 7, paddingHorizontal: 8, paddingVertical: 8, textAlign: "right", fontSize: 13 },
  button: { backgroundColor: colors.accentCyan, borderRadius: 8, alignItems: "center", paddingVertical: 13, marginTop: 18 },
  disabled: { opacity: 0.55 },
  cycleButtons: { flexDirection: "row", gap: 8, marginTop: 14 },
  cycleButton: { flex: 1, backgroundColor: colors.accentCyan, borderRadius: 8, alignItems: "center", justifyContent: "center", paddingVertical: 12 },
  secondaryButton: { flex: 1, backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 8, alignItems: "center", justifyContent: "center", paddingVertical: 12 },
  secondaryButtonText: { color: colors.textPrimary, fontSize: 11, fontWeight: "800" },
  cycleSafety: { color: colors.accentCyan, fontSize: 9, fontWeight: "800", marginTop: 12, letterSpacing: 0.6 },
  mirrorEntry: { paddingVertical: 6, borderBottomWidth: 1, borderBottomColor: colors.border },
  buttonText: { color: colors.bgPrimary, fontSize: 12, fontWeight: "800" },
  error: { color: colors.bearish, fontSize: 11, marginTop: 10 },
  decision: { fontSize: 30, fontWeight: "900", letterSpacing: 2, marginVertical: 8 },
  metricRow: { flexDirection: "row", justifyContent: "space-between", gap: 12, paddingVertical: 8, borderBottomWidth: 1, borderBottomColor: colors.border },
  metricLabel: { color: colors.textMuted, fontSize: 11 },
  metricValue: { color: colors.white, fontSize: 11, fontWeight: "700", maxWidth: "58%", textAlign: "right" },
  rationale: { color: colors.textMuted, fontSize: 11, lineHeight: 16, marginTop: 12 },
  empty: { color: colors.textMuted, fontSize: 12, lineHeight: 18, marginTop: 6 },
  safety: { borderLeftWidth: 2, borderLeftColor: colors.accentCyan, paddingLeft: 12, marginTop: 2 },
  safetyTitle: { color: colors.accentCyan, fontSize: 11, fontWeight: "800" },
  safetyBody: { color: colors.textMuted, fontSize: 11, lineHeight: 17, marginTop: 6 },
});
