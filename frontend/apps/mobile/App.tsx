import React, { useEffect, useState } from "react";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { View, Pressable, Text, StyleSheet } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { getReactNativePersistence } from "firebase/auth";
import { colors } from "./src/theme";
import { LoginScreen } from "./src/screens/LoginScreen";
import { ArchiveExploreScreen } from "./src/screens/ArchiveExploreScreen";
import { initializeQoinFirebase, subscribeQoinAuth } from "@qoin/core";

export type RootStackParamList = { Login: undefined; Dashboard: undefined };
const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();
type MainTab = "Archive" | "Explore";

function TabBar({ current, onChange }: { current: MainTab; onChange: (tab: MainTab) => void }) {
  return (
    <View style={styles.tabBar}>
      {(["Archive", "Explore"] as MainTab[]).map((tab) => (
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
        <ArchiveExploreScreen mode={tab} />
      </View>
      <TabBar current={tab} onChange={setTab} />
    </View>
  );
}

export default function App() {
  const [authReady, setAuthReady] = useState(false);
  const [signedIn, setSignedIn] = useState(false);
  const [authError, setAuthError] = useState("");
  useEffect(() => {
    const config = {
      apiKey: process.env.EXPO_PUBLIC_FIREBASE_API_KEY,
      authDomain: process.env.EXPO_PUBLIC_FIREBASE_AUTH_DOMAIN,
      projectId: process.env.EXPO_PUBLIC_FIREBASE_PROJECT_ID,
      storageBucket: process.env.EXPO_PUBLIC_FIREBASE_STORAGE_BUCKET,
      messagingSenderId: process.env.EXPO_PUBLIC_FIREBASE_MESSAGING_SENDER_ID,
      appId: process.env.EXPO_PUBLIC_FIREBASE_APP_ID,
    };
    if (!config.apiKey || !config.authDomain || !config.projectId || !config.storageBucket || !config.messagingSenderId || !config.appId) {
      setAuthError("Firebase is not configured. Add the EXPO_PUBLIC_FIREBASE_* values before signing in.");
      setAuthReady(true);
      return;
    }
    try {
      initializeQoinFirebase(config, getReactNativePersistence(AsyncStorage));
      return subscribeQoinAuth((user) => {
        setSignedIn(Boolean(user));
        setAuthReady(true);
      });
    } catch (error) {
      setAuthError(error instanceof Error ? error.message : "Firebase initialization failed.");
      setAuthReady(true);
      return undefined;
    }
  }, []);

  return (
    <QueryClientProvider client={queryClient}>
      <SafeAreaProvider>
        <NavigationContainer>
          <StatusBar style="dark" backgroundColor={colors.bgPrimary} />
          {authReady ? (
            <Stack.Navigator screenOptions={{ headerShown: false }}>
              {signedIn ? <Stack.Screen name="Dashboard" component={MainTabs} /> : <Stack.Screen name="Login">
                {(props) => <LoginScreen {...props} configurationError={authError} />}
              </Stack.Screen>}
            </Stack.Navigator>
          ) : null}
        </NavigationContainer>
      </SafeAreaProvider>
    </QueryClientProvider>
  );
}

const styles = StyleSheet.create({
  main: { flex: 1, backgroundColor: colors.bgPrimary },
  screen: { flex: 1 },
  tabBar: { flexDirection: "row", backgroundColor: colors.bgSecondary, borderTopWidth: 0, paddingTop: 9, paddingBottom: 7 },
  tabItem: { flex: 1, alignItems: "center", justifyContent: "center", gap: 5 },
  tabText: { color: colors.textMuted, fontSize: 11, fontWeight: "600" },
  tabTextActive: { color: colors.accentCyan, fontWeight: "800" },
  activeMark: { width: 15, height: 2, borderRadius: 2, backgroundColor: colors.accentCyan },
});
