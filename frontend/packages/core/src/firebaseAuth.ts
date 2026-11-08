import { getApp, getApps, initializeApp, type FirebaseOptions } from "firebase/app";
import {
  createUserWithEmailAndPassword,
  getAuth,
  initializeAuth,
  onAuthStateChanged,
  signInWithEmailAndPassword,
  signOut,
  updateProfile,
  type Auth,
  type Persistence,
  type User,
  type UserCredential,
} from "firebase/auth";
import { doc, getDoc, getFirestore, serverTimestamp, setDoc } from "firebase/firestore";
import { getDownloadURL, getStorage, ref, uploadBytes } from "firebase/storage";

export type QoinFirebaseConfig = FirebaseOptions;
export type QoinProfile = {
  uid: string;
  email: string | null;
  displayName: string | null;
  photoURL: string | null;
};

let authInstance: Auth | null = null;

export function initializeQoinFirebase(config: QoinFirebaseConfig, persistence?: Persistence) {
  const app = getApps().length ? getApp() : initializeApp(config);
  if (!authInstance) {
    try {
      authInstance = persistence ? initializeAuth(app, { persistence }) : getAuth(app);
    } catch (error) {
      // Hot reload may initialize Firebase Auth before this module is evaluated again.
      if (error instanceof Error && error.message.toLowerCase().includes("already")) {
        authInstance = getAuth(app);
      } else {
        throw error;
      }
    }
  }
  return { app, auth: authInstance, db: getFirestore(app), storage: getStorage(app) };
}

export function requireQoinFirebase() {
  if (!authInstance) throw new Error("Firebase is not configured. Set the Qoin Firebase client environment variables.");
  const user = authInstance.currentUser;
  if (!user) throw new Error("Sign in to continue.");
  return { auth: authInstance, user, app: authInstance.app };
}

export function subscribeQoinAuth(callback: (user: User | null) => void) {
  if (!authInstance) throw new Error("Firebase is not configured.");
  return onAuthStateChanged(authInstance, callback);
}

async function saveProfile(user: User): Promise<void> {
  const db = getFirestore(authInstance!.app);
  await setDoc(doc(db, "users", user.uid), {
    uid: user.uid,
    email: user.email,
    displayName: user.displayName,
    photoURL: user.photoURL,
    updatedAt: serverTimestamp(),
  }, { merge: true });
}

export async function registerQoinUser(email: string, password: string): Promise<UserCredential> {
  if (!authInstance) throw new Error("Firebase is not configured.");
  const credential = await createUserWithEmailAndPassword(authInstance, email.trim(), password);
  await saveProfile(credential.user);
  return credential;
}

export async function signInQoinUser(email: string, password: string): Promise<UserCredential> {
  if (!authInstance) throw new Error("Firebase is not configured.");
  const credential = await signInWithEmailAndPassword(authInstance, email.trim(), password);
  await saveProfile(credential.user);
  return credential;
}

export async function signOutQoinUser(): Promise<void> {
  if (!authInstance) throw new Error("Firebase is not configured.");
  await signOut(authInstance);
}

export async function uploadQoinAvatar(file: Blob): Promise<string> {
  const { user } = requireQoinFirebase();
  if (file.size <= 0 || file.size > 8 * 1024 * 1024) throw new Error("Choose an image smaller than 8 MB.");
  if (!new Set(["image/jpeg", "image/png", "image/webp"]).has(file.type.toLowerCase())) {
    throw new Error("Choose a JPEG, PNG, or WebP image.");
  }
  const storage = getStorage(authInstance!.app);
  const object = ref(storage, `users/${user.uid}/avatar`);
  await uploadBytes(object, file, { contentType: file.type });
  const photoURL = await getDownloadURL(object);
  await updateProfile(user, { photoURL });
  await saveProfile(user);
  return photoURL;
}

export function toQoinProfile(user: User): QoinProfile {
  return { uid: user.uid, email: user.email, displayName: user.displayName, photoURL: user.photoURL };
}

export async function getQoinArchivedSymbols(): Promise<string[]> {
  const { user } = requireQoinFirebase();
  const snapshot = await getDoc(doc(getFirestore(authInstance!.app), "users", user.uid));
  const value = snapshot.data()?.archivedSymbols;
  return Array.isArray(value) ? value.filter((symbol): symbol is string => typeof symbol === "string") : [];
}

export async function saveQoinArchivedSymbols(symbols: string[]): Promise<void> {
  const { user } = requireQoinFirebase();
  const archivedSymbols = [...new Set(symbols.map((symbol) => symbol.trim()).filter(Boolean))];
  await setDoc(doc(getFirestore(authInstance!.app), "users", user.uid), {
    archivedSymbols,
    updatedAt: serverTimestamp(),
  }, { merge: true });
}
