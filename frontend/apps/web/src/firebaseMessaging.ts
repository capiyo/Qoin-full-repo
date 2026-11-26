import { getMessaging, getToken, isSupported } from "firebase/messaging";
import { requireQoinFirebase } from "@qoin/core";

export async function requestQoinWebPushToken(): Promise<string | null> {
  if (typeof window === "undefined" || !("Notification" in window) || !("serviceWorker" in navigator)) return null;
  if (!(await isSupported())) return null;
  const vapidKey = import.meta.env.VITE_FIREBASE_VAPID_KEY;
  if (!vapidKey) throw new Error("Set VITE_FIREBASE_VAPID_KEY to enable browser push.");
  const permission = await Notification.requestPermission();
  if (permission !== "granted") return null;
  const { app } = requireQoinFirebase();
  const options = app.options;
  const params = new URLSearchParams({
    apiKey: String(options.apiKey ?? ""),
    authDomain: String(options.authDomain ?? ""),
    projectId: String(options.projectId ?? ""),
    appId: String(options.appId ?? ""),
    messagingSenderId: String(options.messagingSenderId ?? ""),
  });
  const registration = await navigator.serviceWorker.register(`/firebase-messaging-sw.js?${params.toString()}`);
  const token = await getToken(getMessaging(app), { vapidKey, serviceWorkerRegistration: registration });
  return token || null;
}
