importScripts("https://www.gstatic.com/firebasejs/10.14.1/firebase-app-compat.js");
importScripts("https://www.gstatic.com/firebasejs/10.14.1/firebase-messaging-compat.js");
const p=new URL(self.location.href).searchParams;
firebase.initializeApp({apiKey:p.get("apiKey"),authDomain:p.get("authDomain"),projectId:p.get("projectId"),appId:p.get("appId"),messagingSenderId:p.get("messagingSenderId")});
const messaging=firebase.messaging();
messaging.onBackgroundMessage(payload=>{const n=payload.notification||{};self.registration.showNotification(n.title||"Qoin alert",{body:n.body||"",icon:"/favicon.ico",data:payload.data||{}});});
