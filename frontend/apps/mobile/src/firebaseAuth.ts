import AsyncStorage from "@react-native-async-storage/async-storage";
import { getApp, getApps, initializeApp } from "firebase/app";
import {
  getAuth,
  getReactNativePersistence,
  initializeAuth,
  type Auth,
} from "firebase/auth";

const config = {
  apiKey: process.env.EXPO_PUBLIC_FIREBASE_API_KEY,
  authDomain: process.env.EXPO_PUBLIC_FIREBASE_AUTH_DOMAIN,
  projectId: process.env.EXPO_PUBLIC_FIREBASE_PROJECT_ID,
  appId: process.env.EXPO_PUBLIC_FIREBASE_APP_ID,
};

export const isFirebaseConfigured = Object.values(config).every(
  (value) => typeof value === "string" && value.length > 0,
);

let configuredAuth: Auth | null = null;
if (isFirebaseConfigured) {
  const app = getApps().length > 0 ? getApp() : initializeApp(config);
  try {
    configuredAuth = initializeAuth(app, {
      persistence: getReactNativePersistence(AsyncStorage),
    });
  } catch {
    // Fast refresh may initialize the same Firebase app more than once.
    configuredAuth = getAuth(app);
  }
}

export const firebaseAuth = configuredAuth;
