import React, { useEffect, useState } from "react";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator, type NativeStackScreenProps } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { Animated, View, Pressable, StyleSheet } from "react-native";
import Svg, { Path, Circle } from "react-native-svg";
import { useFonts, DMSans_400Regular, DMSans_500Medium, DMSans_700Bold } from "@expo-google-fonts/dm-sans";
import { SafeAreaProvider } from "react-native-safe-area-context";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { getReactNativePersistence } from "firebase/auth";
import { LoginScreen } from "./src/screens/LoginScreen";
import { ArchiveExploreScreen } from "./src/screens/ArchiveExploreScreen";
import { initializeQoinFirebase, subscribeQoinAuth } from "@qoin/core";

export type RootStackParamList = { Login: { mode?: "signin" | "register" } | undefined; Dashboard: undefined };
const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();
type MainTab = "Archive" | "Explore";
type MainTabsProps = NativeStackScreenProps<RootStackParamList, "Dashboard">;

function TabBar({ current, onChange, visible }: { current: MainTab; onChange: (tab: MainTab) => void; visible: boolean }) {
  return (
    <Animated.View pointerEvents={visible ? "auto" : "none"} style={[styles.tabBar, { opacity: visible ? 1 : 0, transform: [{ translateY: visible ? 0 : 72 }] }]} accessibilityRole="tablist">
      {(["Archive", "Explore"] as MainTab[]).map((tab) => {
        const active = current === tab;
        return (
          <Pressable key={tab} accessibilityRole="tab" accessibilityLabel={tab} accessibilityState={{ selected: active }} onPress={() => onChange(tab)} style={[styles.tabItem, active && styles.tabItemActive]}>
            {tab === "Archive" ? (
              <Svg width={19} height={19} viewBox="0 0 24 24">
                <Path d="M4 7.5h16v12H4zM3 4.5h18v3H3zM9 11.5h6" fill="none" stroke={active ? "#061308" : "#89968D"} strokeWidth={1.8} strokeLinecap="round" strokeLinejoin="round" />
              </Svg>
            ) : (
              <Svg width={19} height={19} viewBox="0 0 24 24">
                <Circle cx={10.8} cy={10.8} r={6.3} fill="none" stroke={active ? "#061308" : "#89968D"} strokeWidth={1.8} />
                <Path d="m15.5 15.5 4.2 4.2" fill="none" stroke={active ? "#061308" : "#89968D"} strokeWidth={1.8} strokeLinecap="round" />
              </Svg>
            )}
          </Pressable>
        );
      })}
    </Animated.View>
  );
}

function MainTabs({ navigation }: MainTabsProps) {
  const [tab, setTab] = useState<MainTab>("Archive");
  const [chromeVisible, setChromeVisible] = useState(true);
  return (
    <View style={styles.main}>
      <View style={styles.screen}>
        <ArchiveExploreScreen mode={tab} onAuthRequested={(mode) => navigation.navigate("Login", { mode })} onHeaderVisibilityChange={setChromeVisible} />
      </View>
      <TabBar current={tab} onChange={(nextTab) => { setChromeVisible(true); setTab(nextTab); }} visible={chromeVisible} />
    </View>
  );
}

export default function App() {
  useFonts({ DMSans_400Regular, DMSans_500Medium, DMSans_700Bold });
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
      return subscribeQoinAuth((user) => { setSignedIn(Boolean(user)); setAuthReady(true); });
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
          <StatusBar style="light" backgroundColor="#0E1612" />
          {authReady ? (
            <Stack.Navigator screenOptions={{ headerShown: false }}>
              {signedIn ? (
                <Stack.Screen name="Dashboard" component={MainTabs} />
              ) : (
                <Stack.Screen name="Login">
                  {(props) => <LoginScreen {...props} configurationError={authError} />}
                </Stack.Screen>
              )}
            </Stack.Navigator>
          ) : null}
        </NavigationContainer>
      </SafeAreaProvider>
    </QueryClientProvider>
  );
}

const styles = StyleSheet.create({
  main: { flex: 1, backgroundColor: "#0E1612" },
  screen: { flex: 1 },
  tabBar: { position: "absolute", alignSelf: "center", bottom: 16, flexDirection: "row", alignItems: "center", gap: 4, backgroundColor: "#17291F", borderWidth: 0, borderRadius: 999, padding: 4, zIndex: 20, elevation: 6, shadowColor: "#000000", shadowOpacity: 0.2, shadowRadius: 10, shadowOffset: { width: 0, height: 4 } },
  tabItem: { width: 44, height: 44, alignItems: "center", justifyContent: "center", borderRadius: 999, backgroundColor: "transparent" },
  tabItemActive: { backgroundColor: "#7DF162" },
});
