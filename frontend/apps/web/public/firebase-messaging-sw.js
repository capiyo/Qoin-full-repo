importScripts("https://www.gstatic.com/firebasejs/10.14.1/firebase-app-compat.js");
importScripts("https://www.gstatic.com/firebasejs/10.14.1/firebase-messaging-compat.js");
const params = new URL(self.location.href).searchParams;
firebase.initializeApp({
  apiKey: params.get("apiKey"),
  authDomain: params.get("authDomain"),
  projectId: params.get("projectId"),
  appId: params.get("appId"),
  messagingSenderId: params.get("messagingSenderId"),
});
const messaging = firebase.messaging();
messaging.onBackgroundMessage((payload) => {
  const notification = payload.notification || {};
  self.registration.showNotification(notification.title || "Qoin alert", {
    body: notification.body || "",
    icon: "/favicon.ico",
    data: payload.data || {},
  });
});
