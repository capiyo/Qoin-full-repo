import React, { useState } from "react";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { View, Pressable, Text, StyleSheet } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { colors } from "./src/theme";
import { LoginScreen } from "./src/screens/LoginScreen";
import { SettingsScreen } from "./src/screens/SettingsScreen";
import { SignalScreen } from "./src/screens/SignalScreen";
import { ArchiveExploreScreen } from "./src/screens/ArchiveExploreScreen";

export type RootStackParamList = {
  Login: undefined;
  Dashboard: undefined;
};

const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();
type MainTab = "Archive" | "Explore" | "Signal" | "Settings";

function TabBar({ current, onChange }: { current: MainTab; onChange: (tab: MainTab) => void }) {
  const tabs: MainTab[] = ["Archive", "Explore", "Signal", "Settings"];
  return (
    <View style={styles.tabBar}>
      {tabs.map((tab) => (
        <Pressable key={tab} onPress={() => onChange(tab)} style={styles.tabItem}>
          <Text style={[styles.tabText, current === tab && styles.tabTextActive]}>{tab}</Text>
          {current === tab ? <View style={styles.activeMark} /> : null}
        </Pressable>
      ))}
    </View>
  );
}

function MainTabs() {
  const [tab, setTab] = useState<MainTab>("Archive");
  return (
    <View style={styles.main}>
      <View style={styles.screen}>
        {tab === "Archive" || tab === "Explore" ? <ArchiveExploreScreen mode={tab} /> : null}
        {tab === "Signal" ? <SignalScreen /> : null}
        {tab === "Settings" ? <SettingsScreen /> : null}
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
          <StatusBar style="dark" backgroundColor={colors.bgPrimary} />
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
  main: { flex: 1, backgroundColor: colors.bgPrimary },
  screen: { flex: 1 },
  tabBar: {
    flexDirection: "row",
    backgroundColor: colors.bgSecondary,
    borderTopWidth: 1,
    borderTopColor: colors.border,
    paddingTop: 9,
    paddingBottom: 7,
  },
  tabItem: { flex: 1, alignItems: "center", justifyContent: "center", gap: 5 },
  tabText: { color: colors.textMuted, fontSize: 11, fontWeight: "600" },
  tabTextActive: { color: colors.accentCyan, fontWeight: "800" },
  activeMark: { width: 15, height: 2, borderRadius: 2, backgroundColor: colors.accentCyan },
});
