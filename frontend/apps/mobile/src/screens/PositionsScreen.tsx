import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../theme";

export function PositionsScreen() {
  return (
    <SafeAreaView style={styles.screen}>
      <Text style={styles.title}>Positions</Text>
      <View style={styles.empty}>
        <Text style={styles.heading}>Position tracking is not connected</Text>
        <Text style={styles.copy}>
          Qoin is currently a research preview. It does not hold broker positions, calculate realized profit and loss, or submit orders.
        </Text>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, padding: 16 },
  title: { color: colors.white, fontSize: 20, fontWeight: "700", marginBottom: 12 },
  empty: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 10, padding: 16 },
  heading: { color: colors.textPrimary, fontSize: 13, fontWeight: "700", marginBottom: 6 },
  copy: { color: colors.textMuted, fontSize: 12, lineHeight: 18 },
});
