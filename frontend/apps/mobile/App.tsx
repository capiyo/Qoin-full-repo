import React, { useState } from "react";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { View, Pressable, Text, StyleSheet } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { colors } from "./src/theme";
import { LoginScreen } from "./src/screens/LoginScreen";
import { DashboardScreen } from "./src/screens/DashboardScreen";
import { PositionsScreen } from "./src/screens/PositionsScreen";
import { SettingsScreen } from "./src/screens/SettingsScreen";
import { SignalScreen } from "./src/screens/SignalScreen";

export type RootStackParamList = {
  Login: undefined;
  Dashboard: undefined;
  Positions: undefined;
  Settings: undefined;
};

const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();

// Simple bottom tab bar hand-rolled to avoid an extra nav dependency;
// swap for @react-navigation/bottom-tabs if you want native tab transitions.
function TabBar({ current, onChange }: { current: "Dashboard" | "Positions" | "Signal" | "Settings"; onChange: (t: "Dashboard" | "Positions" | "Signal" | "Settings") => void }) {
  const tabs: Array<"Dashboard" | "Positions" | "Signal" | "Settings"> = ["Dashboard", "Positions", "Signal", "Settings"];
  return (
    <View style={styles.tabBar}>
      {tabs.map((t) => (
        <Pressable key={t} onPress={() => onChange(t)} style={styles.tabItem}>
          <Text style={{ color: current === t ? colors.accentCyan : colors.textMuted, fontWeight: "600", fontSize: 12 }}>
            {t}
          </Text>
        </Pressable>
      ))}
    </View>
  );
}

function MainTabs() {
  const [tab, setTab] = useState<"Dashboard" | "Positions" | "Signal" | "Settings">("Dashboard");
  return (
    <View style={{ flex: 1, backgroundColor: colors.bgPrimary }}>
      <View style={{ flex: 1 }}>
        {tab === "Dashboard" && <DashboardScreen />}
        {tab === "Positions" && <PositionsScreen />}
        {tab === "Signal" && <SignalScreen />}
        {tab === "Settings" && <SettingsScreen />}
      </View>
      <TabBar current={tab} onChange={setTab} />
    </View>
  );
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <SafeAreaProvider>
        <NavigationContainer>
          <StatusBar style="light" backgroundColor={colors.bgPrimary} />
          <Stack.Navigator screenOptions={{ headerShown: false }}>
            <Stack.Screen name="Login" component={LoginScreen} />
            <Stack.Screen name="Dashboard" component={MainTabs} />
          </Stack.Navigator>
        </NavigationContainer>
      </SafeAreaProvider>
    </QueryClientProvider>
  );
}

const styles = StyleSheet.create({
  tabBar: {
    flexDirection: "row",
    borderTopWidth: 1,
    borderTopColor: colors.border,
    backgroundColor: colors.bgSecondary,
    paddingVertical: 10,
  },
  tabItem: { flex: 1, alignItems: "center" },
});
