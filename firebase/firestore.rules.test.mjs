import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { initializeTestEnvironment, assertFails, assertSucceeds } from "@firebase/rules-unit-testing";
import { doc, getDoc, serverTimestamp, setDoc } from "firebase/firestore";

const projectId = "demo-qoin-firestore-rules";
const rules = await readFile(new URL("./firestore.rules", import.meta.url), "utf8");
const environment = await initializeTestEnvironment({
  projectId,
  firestore: { rules },
});

try {
  const owner = environment.authenticatedContext("user-1", { email: "owner@example.test" }).firestore();
  const other = environment.authenticatedContext("user-2").firestore();
  const anonymous = environment.unauthenticatedContext().firestore();
  const ownerRef = doc(owner, "users", "user-1");
  const otherRef = doc(other, "users", "user-1");

  await assertFails(getDoc(doc(anonymous, "users", "user-1")));
  await assertSucceeds(setDoc(ownerRef, {
    uid: "user-1",
    email: "owner@example.test",
    displayName: "Owner",
    photoURL: null,
    updatedAt: serverTimestamp(),
  }));
  await assertSucceeds(getDoc(ownerRef));
  await assertFails(getDoc(otherRef));
  await assertFails(setDoc(otherRef, { archivedSymbols: ["BTCUSDT"], updatedAt: serverTimestamp() }, { merge: true }));
  await assertSucceeds(setDoc(ownerRef, {
    archivedSymbols: ["BTCUSDT", "ETHUSDT"],
    updatedAt: serverTimestamp(),
  }, { merge: true }));
  await assertFails(setDoc(ownerRef, { role: "admin" }, { merge: true }));
  await assertFails(setDoc(ownerRef, {
    archivedSymbols: Array.from({ length: 501 }, (_, index) => `SYMBOL${index}`),
    updatedAt: serverTimestamp(),
  }, { merge: true }));
  await assertFails(setDoc(ownerRef, {
    displayName: "x".repeat(121),
    updatedAt: serverTimestamp(),
  }, { merge: true }));

  console.log("Firestore security rules tests passed.");
} finally {
  await environment.cleanup();
}
