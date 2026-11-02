import { getApp, getApps, initializeApp } from "firebase/app";
import { getAuth } from "firebase/auth";
import { getMessaging, getToken, isSupported, onMessage, type MessagePayload } from "firebase/messaging";

const config = {
  apiKey: import.meta.env.VITE_FIREBASE_API_KEY || "AIzaSyCWMGz6AIRXgu7GVZiJWlkvO6tVZADf5tY",
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN || "funzy-d56d7.firebaseapp.com",
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID || "funzy-d56d7",
  appId: import.meta.env.VITE_FIREBASE_APP_ID || "1:661929781606:web:3f306a659ae64ac7f780a7",
  messagingSenderId: import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID || "661929781606",
};
export function getFirebaseAuth() {
  if (!config.apiKey || !config.projectId || !config.appId) throw new Error("Firebase phone authentication is not configured. Set the VITE_FIREBASE_* environment variables.");
  const app = getApps().length ? getApp() : initializeApp(config);
  return getAuth(app);
}
export async function requestWebPushToken(): Promise<string | null> {
  const vapid = import.meta.env.VITE_FIREBASE_VAPID_KEY || "BIvcsdfnoQ07A2PAEiXvHfjLPOfyga-fiPB-JLJfdr7NbXxwWJMr6fNT-71RzUVP-WZcL76W_sN137Fs9wMhi90";
  if (!vapid || !("Notification" in window) || !(await isSupported())) return null;
  const permission = await Notification.requestPermission();
  if (permission !== "granted") return null;
  const app = getApps().length ? getApp() : initializeApp(config);
  const params = new URLSearchParams({ apiKey: config.apiKey, authDomain: config.authDomain, projectId: config.projectId, appId: config.appId, messagingSenderId: config.messagingSenderId });
  const registration = await navigator.serviceWorker.register(`/firebase-messaging-sw.js?${params.toString()}`);
  return (await getToken(getMessaging(app), { vapidKey: vapid, serviceWorkerRegistration: registration })) || null;
}

export function onForegroundMessage(callback:(payload:MessagePayload)=>void):()=>void {
  if (typeof window === "undefined") return () => {};
  try {
    const app = getApps().length ? getApp() : initializeApp(config);
    return onMessage(getMessaging(app), callback);
  } catch {
    return () => {};
  }
}
