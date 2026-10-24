import React from "react";
import { View, Text, StyleSheet } from "react-native";
import type { FeedStatus } from "@qoin/core";
import { colors } from "../theme";

export function FeedBanner({ status }: { status: FeedStatus }) {
  if (status === "live" || status === "connecting") return null;
  const isStale = status === "stale";
  return (
    <View
      style={[
        styles.banner,
        { borderColor: isStale ? colors.warning : colors.bearish, backgroundColor: isStale ? "#F59E0B1A" : "#EF44441A" },
      ]}
    >
      <Text style={{ color: isStale ? colors.warning : colors.bearish, fontSize: 12, fontWeight: "600" }}>
        {isStale ? "Price data may be delayed — last update over 5s ago." : "Disconnected. Reconnecting…"}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  banner: { borderWidth: 1, borderRadius: 10, padding: 10, marginBottom: 12 },
});
