import React, { useState } from "react";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { View, Pressable, Text, StyleSheet } from "react-native";
import { SafeAreaProvider } from "react-native-safe-area-context";
import { colors } from "./src/theme";
import { LoginScreen } from "./src/screens/LoginScreen";
import { ArchiveExploreScreen } from "./src/screens/ArchiveExploreScreen";

export type RootStackParamList = {
  Login: undefined;
  Dashboard: undefined;
  Positions: undefined;
  Settings: undefined;
};

const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();

function MainTabs() {
  return <ArchiveExploreScreen />;
}

