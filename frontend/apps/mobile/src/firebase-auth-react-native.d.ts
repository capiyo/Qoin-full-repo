// Firebase 10.14's browser declaration omits the React Native-only export.
// Metro resolves firebase/auth to Firebase's React Native entry point at runtime.
import "firebase/auth";

declare module "firebase/auth" {
  export function getReactNativePersistence(
    storage: typeof import("@react-native-async-storage/async-storage").default,
  ): import("firebase/auth").Persistence;
}
