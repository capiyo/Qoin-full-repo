import React, { useState } from "react";
import { View, Text, ScrollView, Pressable, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { WATCHLIST } from "@qoin/core";
import { colors } from "../theme";
import { useMarketFeed } from "../useMarketFeed";
import { StatusBadge } from "../components/StatusBadge";
import { CandleChart } from "../components/CandleChart";
import { FeedBanner } from "../components/FeedBanner";
import { OrderTicket } from "../components/OrderTicket";

function fmt(n: number) {
  return n.toLocaleString(undefined, { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

export function DashboardScreen() {
  const { status, candlesBySymbol, quotes } = useMarketFeed();
  const [active, setActive] = useState<string>(WATCHLIST[0]);
  const q = quotes[active];
  const up = q.changePct24h >= 0;

  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.header}>
        <Text style={styles.title}>Dashboard</Text>
        <StatusBadge status={status} />
      </View>

      <FeedBanner status={status} />

      <ScrollView horizontal showsHorizontalScrollIndicator={false} style={styles.watchlist}>
        {WATCHLIST.map((symbol) => {
          const qt = quotes[symbol];
          const itemUp = qt.changePct24h >= 0;
          const isActive = symbol === active;
          return (
            <Pressable
              key={symbol}
              onPress={() => setActive(symbol)}
              style={[styles.chip, isActive && styles.chipActive]}
            >
              <Text style={styles.chipSymbol}>{symbol}</Text>
              <Text style={[styles.chipPct, { color: itemUp ? colors.bullish : colors.bearish }]}>
                {itemUp ? "+" : ""}
                {qt.changePct24h.toFixed(2)}%
              </Text>
            </Pressable>
          );
        })}
      </ScrollView>

      <ScrollView showsVerticalScrollIndicator={false}>
        <View style={styles.card}>
          <Text style={styles.symbol}>{active}</Text>
          <Text style={styles.price}>${fmt(q.price)}</Text>
          <Text style={{ color: up ? colors.bullish : colors.bearish, fontWeight: "600", marginBottom: 8 }}>
            {up ? "+" : ""}
            {q.changePct24h.toFixed(2)}% (24h)
          </Text>
          <CandleChart candles={candlesBySymbol[active]} width={320} height={200} />
        </View>
        <OrderTicket symbol={active} quote={q} />
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, padding: 16 },
  header: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginBottom: 12 },
  title: { color: colors.white, fontSize: 20, fontWeight: "700" },
  watchlist: { flexGrow: 0, marginBottom: 12 },
  chip: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 10, padding: 10, marginRight: 8, minWidth: 100 },
  chipActive: { borderColor: colors.accentCyan },
  chipSymbol: { color: colors.textPrimary, fontSize: 12, fontWeight: "600" },
  chipPct: { fontSize: 12, marginTop: 2, fontWeight: "600" },
  card: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 12, padding: 16 },
  symbol: { color: colors.textMuted, fontSize: 13, fontWeight: "600" },
  price: { color: colors.white, fontSize: 28, fontWeight: "700" },
});
