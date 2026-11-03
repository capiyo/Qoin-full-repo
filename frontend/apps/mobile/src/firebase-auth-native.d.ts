import type { Persistence } from "firebase/auth";
import type AsyncStorage from "@react-native-async-storage/async-storage";

declare module "firebase/auth" {
  export function getReactNativePersistence(storage: typeof AsyncStorage): Persistence;
}

declare global {
  const process: {
    env: {
      EXPO_PUBLIC_FIREBASE_API_KEY?: string;
      EXPO_PUBLIC_FIREBASE_AUTH_DOMAIN?: string;
      EXPO_PUBLIC_FIREBASE_PROJECT_ID?: string;
      EXPO_PUBLIC_FIREBASE_APP_ID?: string;
    };
  };
}
