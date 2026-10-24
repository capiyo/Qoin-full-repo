import React from "react";
import { View, Text, FlatList, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../theme";
import { useMarketFeed, usePositions } from "../useMarketFeed";
import type { Position } from "@qoin/core";

function fmt(n: number, digits = 2) {
  return n.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

function Row({ p }: { p: Position }) {
  const positive = p.pnl >= 0;
  return (
    <View style={styles.row}>
      <View style={{ flex: 1 }}>
        <Text style={styles.symbol}>{p.symbol}</Text>
        <Text style={[styles.side, { color: p.side === "buy" ? colors.bullish : colors.bearish }]}>
          {p.side.toUpperCase()} · {p.leverage}x
        </Text>
      </View>
      <View style={{ alignItems: "flex-end" }}>
        <Text style={styles.mark}>${fmt(p.markPrice)}</Text>
        <Text style={{ color: positive ? colors.bullish : colors.bearish, fontWeight: "600" }}>
          {positive ? "+" : ""}
          {fmt(p.pnl)} ({positive ? "+" : ""}
          {fmt(p.pnlPct)}%)
        </Text>
      </View>
    </View>
  );
}

export function PositionsScreen() {
  const { quotes } = useMarketFeed();
  const { data: positions } = usePositions(quotes);

  return (
    <SafeAreaView style={styles.screen}>
      <Text style={styles.title}>Positions</Text>
      <FlatList
        data={positions ?? []}
        keyExtractor={(p) => p.id}
        renderItem={({ item }) => <Row p={item} />}
        ListEmptyComponent={<Text style={{ color: colors.textMuted }}>Loading positions…</Text>}
      />
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, padding: 16 },
  title: { color: colors.white, fontSize: 20, fontWeight: "700", marginBottom: 12 },
  row: {
    flexDirection: "row",
    justifyContent: "space-between",
    backgroundColor: colors.bgSecondary,
    borderWidth: 1,
    borderColor: colors.border,
    borderRadius: 10,
    padding: 12,
    marginBottom: 8,
  },
  symbol: { color: colors.textPrimary, fontWeight: "700" },
  side: { fontSize: 12, fontWeight: "600", marginTop: 2 },
  mark: { color: colors.textPrimary, fontWeight: "600" },
});
