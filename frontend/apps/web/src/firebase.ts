import { getApp, getApps, initializeApp } from "firebase/app";
import { getAuth } from "firebase/auth";
import { getMessaging, getToken, isSupported } from "firebase/messaging";

const config = {
  apiKey: import.meta.env.VITE_FIREBASE_API_KEY,
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN,
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID,
  appId: import.meta.env.VITE_FIREBASE_APP_ID,
  messagingSenderId: import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID,
};
export function getFirebaseAuth() {
  if (!config.apiKey || !config.projectId || !config.appId) throw new Error("Firebase phone authentication is not configured. Set the VITE_FIREBASE_* environment variables.");
  const app = getApps().length ? getApp() : initializeApp(config);
  return getAuth(app);
}
export async function requestWebPushToken(): Promise<string | null> {
  const vapid = import.meta.env.VITE_FIREBASE_VAPID_KEY;
  if (!vapid || !("Notification" in window) || !(await isSupported())) return null;
  const permission = await Notification.requestPermission();
  if (permission !== "granted") return null;
  const app = getApps().length ? getApp() : initializeApp(config);
  const params = new URLSearchParams({ apiKey: config.apiKey, authDomain: config.authDomain, projectId: config.projectId, appId: config.appId, messagingSenderId: config.messagingSenderId });\n  const registration = await navigator.serviceWorker.register(`/firebase-messaging-sw.js?${params.toString()}`);
  return (await getToken(getMessaging(app), { vapidKey: vapid, serviceWorkerRegistration: registration })) || null;
}
