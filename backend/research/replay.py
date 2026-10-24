"""Deterministic event-driven research replay. Never submits exchange orders."""
from __future__ import annotations

from dataclasses import dataclass, field
from math import isfinite, sqrt
from typing import Callable, Iterable, Literal

Action = Literal["buy", "sell", "close", "wait"]
Strategy = Callable[["ReplayEvent"], Action]


@dataclass(frozen=True)
class ReplayEvent:
    event_ts_ms: int
    arrival_ts_ms: int
    symbol: str
    event_type: str
    mid_price: float
    bid: float
    ask: float
    ofi: float = 0.0
    funding_rate: float = 0.0
    volatility: float = 0.0
    sequence: int | None = None
    regime_allowed: bool = False
    reject_order: bool = False
    fill_ratio: float = 1.0
    sequence_reset: bool = False

    def validate(self) -> None:
        if self.event_ts_ms <= 0 or self.arrival_ts_ms < self.event_ts_ms:
            raise ValueError("event and arrival timestamps must be positive and causal")
        if not self.symbol or self.event_type not in {"trade", "book", "funding", "open_interest", "liquidation"}:
            raise ValueError("invalid symbol or event type")
        numbers = (self.mid_price, self.bid, self.ask, self.ofi, self.funding_rate, self.volatility, self.fill_ratio)
        if any(not isfinite(float(value)) for value in numbers):
            raise ValueError("all numeric event values must be finite")
        if self.mid_price <= 0 or self.bid <= 0 or self.ask < self.bid:
            raise ValueError("invalid market prices")
        if not -1.0 <= self.fill_ratio <= 1.0:
            raise ValueError("fill_ratio must be in [-1, 1]")


@dataclass(frozen=True)
class ReplayConfig:
    initial_equity: float = 10_000.0
    risk_per_trade_fraction: float = 0.0025
    max_leverage: float = 2.0
    stop_distance_bps: float = 100.0
    fee_bps: float = 4.0
    base_slippage_bps: float = 2.0
    volatile_slippage_multiplier: float = 4.0
    volatility_threshold: float = 0.01
    total_cost_stress_multiplier: float = 1.0
    max_spread_bps: float = 25.0
    max_feed_age_ms: int = 5_000

    def validate(self) -> None:
        values = (self.initial_equity, self.risk_per_trade_fraction, self.max_leverage,
                  self.stop_distance_bps, self.fee_bps, self.base_slippage_bps,
                  self.volatile_slippage_multiplier, self.volatility_threshold,
                  self.total_cost_stress_multiplier, self.max_spread_bps)
        if any(not isfinite(v) for v in values) or self.initial_equity <= 0:
            raise ValueError("replay configuration must contain finite values and positive equity")
        if not 0 < self.risk_per_trade_fraction <= 0.0025 or not 0 < self.max_leverage <= 2.0:
            raise ValueError("replay risk limits may not exceed the hard 0.25% / 2x policy")
        if self.stop_distance_bps <= 0 or self.max_feed_age_ms <= 0 or self.max_spread_bps <= 0:
            raise ValueError("stop distance, feed age, and spread cap must be positive")
        if self.total_cost_stress_multiplier < 1.0 or self.volatile_slippage_multiplier < 1.0:
            raise ValueError("cost multipliers cannot reduce costs")


@dataclass
class Position:
    symbol: str
    side: int
    quantity: float
    entry_price: float
    stop_price: float
    entry_fee: float


@dataclass
class ReplayResult:
    initial_equity: float
    final_equity: float
    trade_count: int
    wins: int
    losses: int
    fees_paid: float
    spread_cost: float
    slippage_cost: float
    funding_pnl: float
    rejected_orders: int
    stale_events: int
    sequence_rejections: int
    equity_curve: list[float] = field(default_factory=list)
    trade_pnls: list[float] = field(default_factory=list)

    @property
    def total_return(self) -> float:
        return self.final_equity / self.initial_equity - 1.0

    @property
    def max_drawdown(self) -> float:
        peak = self.initial_equity
        worst = 0.0
        for equity in self.equity_curve:
            peak = max(peak, equity)
            if peak > 0:
                worst = max(worst, (peak - equity) / peak)
        return worst

    @property
    def profit_factor(self) -> float:
        gains = sum(value for value in self.trade_pnls if value > 0)
        losses = -sum(value for value in self.trade_pnls if value < 0)
        return gains / losses if losses > 0 else (float("inf") if gains > 0 else 0.0)


def _execution_costs(event: ReplayEvent, config: ReplayConfig) -> tuple[float, float, float]:
    spread_bps = (event.ask - event.bid) / event.mid_price * 10_000.0
    volatility_multiplier = config.volatile_slippage_multiplier if event.volatility >= config.volatility_threshold else 1.0
    slippage_bps = config.base_slippage_bps * volatility_multiplier * config.total_cost_stress_multiplier
    effective_spread_bps = spread_bps * config.total_cost_stress_multiplier
    fee_bps = config.fee_bps * config.total_cost_stress_multiplier
    return effective_spread_bps, slippage_bps, fee_bps


def run_replay(events: Iterable[ReplayEvent], strategy: Strategy, config: ReplayConfig = ReplayConfig()) -> ReplayResult:
    """Replay input in arrival order. Out-of-order input raises instead of being silently sorted."""
    config.validate()
    equity = config.initial_equity
    peak_equity = equity
    positions: dict[str, Position] = {}
    last_arrival_ms = 0
    last_prices: dict[str, float] = {}
    last_sequences: dict[tuple[str, str], int] = {}
    invalid_symbols: set[str] = set()
    trade_pnls: list[float] = []
    equity_curve: list[float] = []
    fees_paid = spread_cost = slippage_cost = funding_pnl = 0.0
    trades = wins = losses = rejected = stale = sequence_rejections = 0

    def close_position(event: ReplayEvent, position: Position, fraction: float = 1.0) -> None:
        nonlocal equity, fees_paid, spread_cost, slippage_cost, trades, wins, losses
        quantity = position.quantity * fraction
        if quantity <= 0:
            return
        spread_bps, slip_bps, fee_bps = _execution_costs(event, config)
        side = -position.side
        fill_price = event.mid_price * (1.0 + side * (spread_bps / 20_000.0 + slip_bps / 10_000.0))
        notional = quantity * event.mid_price
        fee = notional * fee_bps / 10_000.0
        spread_paid = notional * spread_bps / 20_000.0
        slippage_paid = notional * slip_bps / 10_000.0
        gross_pnl = quantity * (fill_price - position.entry_price) * position.side
        pnl = gross_pnl - fee
        equity += pnl
        fees_paid += fee
        spread_cost += spread_paid
        slippage_cost += slippage_paid
        trades += 1
        trade_pnls.append(pnl - position.entry_fee * fraction)
        if pnl > 0:
            wins += 1
        elif pnl < 0:
            losses += 1
        position.quantity -= quantity
        if position.quantity <= 1e-12:
            positions.pop(event.symbol, None)

    for event in events:
        event.validate()
        if event.arrival_ts_ms < last_arrival_ms:
            raise ValueError("arrival events must be replayed chronologically")
        last_arrival_ms = event.arrival_ts_ms
        last_prices[event.symbol] = event.mid_price
        key = (event.symbol, event.event_type)
        if event.sequence_reset:
            last_sequences[key] = event.sequence if event.sequence is not None else 0
            invalid_symbols.discard(event.symbol)
        elif event.sequence is None:
            invalid_symbols.add(event.symbol)
            sequence_rejections += 1
        elif key in last_sequences:
            expected = last_sequences[key] + 1
            if event.sequence != expected:
                invalid_symbols.add(event.symbol)
                sequence_rejections += 1
            else:
                last_sequences[key] = event.sequence
        else:
            last_sequences[key] = event.sequence

        age_ms = event.arrival_ts_ms - event.event_ts_ms
        spread_bps = (event.ask - event.bid) / event.mid_price * 10_000.0
        position = positions.get(event.symbol)
        if position and event.event_type == "funding":
            funding = -position.side * position.quantity * event.mid_price * event.funding_rate
            equity += funding
            funding_pnl += funding
            stop_hit = (position.side == 1 and event.mid_price <= position.stop_price) or (
                position.side == -1 and event.mid_price >= position.stop_price
            )
            if stop_hit and not event.reject_order:
                close_position(event, position)
        if age_ms > config.max_feed_age_ms:
            stale += 1
        elif event.event_type in {"trade", "book"} and event.symbol not in invalid_symbols and spread_bps <= config.max_spread_bps:
            action = strategy(event)
            if action in {"buy", "sell", "close"}:
                if event.reject_order or event.fill_ratio <= 0:
                    rejected += 1
                elif action == "close":
                    if position:
                        close_position(event, position, min(event.fill_ratio, 1.0))
                elif event.symbol not in positions and event.regime_allowed:
                    side = 1 if action == "buy" else -1
                    risk_budget = equity * config.risk_per_trade_fraction
                    risk_fraction = config.stop_distance_bps / 10_000.0
                    notional = min(risk_budget / risk_fraction, equity * config.max_leverage)
                    quantity = notional / event.mid_price * event.fill_ratio
                    spread, slip, fee_rate = _execution_costs(event, config)
                    fill_price = event.mid_price * (1.0 + side * (spread / 20_000.0 + slip / 10_000.0))
                    fee = quantity * event.mid_price * fee_rate / 10_000.0
                    equity -= fee
                    fees_paid += fee
                    spread_cost += quantity * event.mid_price * spread / 20_000.0
                    slippage_cost += quantity * event.mid_price * slip / 10_000.0
                    stop_price = fill_price * (1.0 - side * config.stop_distance_bps / 10_000.0)
                    positions[event.symbol] = Position(event.symbol, side, quantity, fill_price, stop_price, fee)
        unrealized = sum(p.quantity * (last_prices.get(p.symbol, p.entry_price) - p.entry_price) * p.side
                         for p in positions.values())
        equity_curve.append(equity + unrealized)
        peak_equity = max(peak_equity, equity)
        if equity <= 0:
            break

    for position in list(positions.values()):
        # Mark remaining positions at the last known market price only in the equity curve;
        # do not fabricate a closing fill at end-of-sample.
        _ = position
    return ReplayResult(config.initial_equity, equity, trades, wins, losses, fees_paid,
                        spread_cost, slippage_cost, funding_pnl, rejected, stale,
                        sequence_rejections, equity_curve, trade_pnls)


def walk_forward_splits(length: int, train_size: int, test_size: int, purge: int, embargo: int) -> list[tuple[range, range]]:
    """Chronological walk-forward splits with a gap between training and test samples."""
    if min(length, train_size, test_size) <= 0 or min(purge, embargo) < 0:
        raise ValueError("split sizes must be positive and gaps non-negative")
    splits = []
    train_start = 0
    while True:
        train_end = train_start + train_size
        test_start = train_end + purge + embargo
        test_end = test_start + test_size
        if test_end > length:
            break
        splits.append((range(train_start, train_end), range(test_start, test_end)))
        train_start += test_size
    return splits


def baseline_strategies() -> dict[str, Strategy]:
    """Simple baselines used only for comparison; none implies a deployable edge."""
    return {
        "no_trade": lambda event: "wait",
        "ofi_only": lambda event: "buy" if event.ofi > 0.5 else "sell" if event.ofi < -0.5 else "wait",
        "linear_baseline": lambda event: "buy" if (0.7 * event.ofi - 0.1 * event.funding_rate) > 0.5
        else "sell" if (0.7 * event.ofi - 0.1 * event.funding_rate) < -0.5 else "wait",
    }


def deflated_sharpe_ratio(returns: list[float], trials: int = 1) -> float | None:
    """Conservative Sharpe-like diagnostic; requires at least 3 finite returns and >=1 trial."""
    if trials < 1 or len(returns) < 3 or any(not isfinite(value) for value in returns):
        return None
    mean = sum(returns) / len(returns)
    variance = sum((value - mean) ** 2 for value in returns) / (len(returns) - 1)
    if variance <= 0:
        return None
    sharpe = mean / sqrt(variance)
    # Multiple-testing penalty grows with the number of tried configurations.
    penalty = sqrt(2.0 * max(0.0, __import__("math").log(trials)))
    return sharpe - penalty
