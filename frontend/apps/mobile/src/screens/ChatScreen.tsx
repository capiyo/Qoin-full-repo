import React, { useState } from "react";
import { Modal, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import Svg, { Path } from "react-native-svg";

type Props = { signedIn: boolean; onAuthRequested: (mode: "signin" | "register") => void };

export function ChatScreen({ signedIn, onAuthRequested }: Props) {
  const [menuOpen, setMenuOpen] = useState(false);
  const [query, setQuery] = useState("");

  return (
    <View style={styles.screen}>
      <View style={styles.header}>
        <Text style={styles.brand}>qoin</Text>
        <View style={styles.actions}>
          <Pressable accessibilityRole="button" accessibilityLabel="Open menu" onPress={() => setMenuOpen(true)} style={styles.menuButton}>
            <Svg width={20} height={20} viewBox="0 0 24 24"><Path d="M4 6h16M4 12h16M4 18h16" fill="none" stroke="#C5D0C7" strokeWidth={1.8} strokeLinecap="round" /></Svg>
          </Pressable>
          <Pressable accessibilityRole="button" accessibilityLabel="Profile and account options" onPress={() => setMenuOpen(true)} style={styles.avatar}>
            <Text style={styles.avatarText}>Q</Text>
          </Pressable>
        </View>
      </View>
      <View style={styles.chatContent}>
        <View style={styles.headingRow}><Text style={styles.heading}>Chats</Text><Text style={styles.meta}>MESSAGES</Text></View>
        <View style={styles.searchBox}><Text style={styles.searchIcon}>⌕</Text><TextInput value={query} onChangeText={setQuery} placeholder="Search chats" placeholderTextColor="#718077" style={styles.searchInput} /></View>
        <ScrollView contentContainerStyle={styles.listContent} keyboardShouldPersistTaps="handled">
          <View style={styles.emptyList}>
            <Text style={styles.emptyIcon}>◌</Text>
            <Text style={styles.emptyTitle}>{query ? "No matching chats" : "No conversations yet"}</Text>
            <Text style={styles.emptyCopy}>{query ? "Try another name or keyword." : "Your conversations will appear here when Qoin messaging is connected."}</Text>
          </View>
        </ScrollView>
        <View style={styles.messagePreview}>
          <View style={styles.messageMark}><Text style={styles.messageMarkText}>q</Text></View>
          <Text style={styles.messageTitle}>Your messages</Text>
          <Text style={styles.messageCopy}>Your most recent conversation will open here. No sample messages or fabricated chat history are shown.</Text>
          <View style={styles.composer}><TextInput editable={false} placeholder="Messaging service not connected" placeholderTextColor="#718077" style={styles.composerInput} /><View style={styles.sendButton}><Text style={styles.sendText}>↑</Text></View></View>
        </View>
      </View>
      <Modal visible={menuOpen} transparent animationType="fade" onRequestClose={() => setMenuOpen(false)}>
        <Pressable style={styles.shade} onPress={() => setMenuOpen(false)}>
          <Pressable style={styles.menuSheet} onPress={(event) => event.stopPropagation()}>
            <Text style={styles.menuTitle}>{signedIn ? "Qoin account" : "Welcome to Qoin"}</Text>
            <Text style={styles.menuCopy}>{signedIn ? "Your account is signed in." : "Sign in to sync your research workspace."}</Text>
            {!signedIn ? <>
              <Pressable style={styles.menuAction} onPress={() => { setMenuOpen(false); onAuthRequested("signin"); }}><Text style={styles.menuActionText}>Sign in</Text></Pressable>
              <Pressable style={styles.menuAction} onPress={() => { setMenuOpen(false); onAuthRequested("register"); }}><Text style={styles.menuActionText}>Create account</Text></Pressable>
            </> : <Text style={styles.menuCopy}>Profile and account management are available from the Explore workspace.</Text>}
            <Pressable style={styles.closeAction} onPress={() => setMenuOpen(false)}><Text style={styles.closeText}>Close</Text></Pressable>
          </Pressable>
        </Pressable>
      </Modal>
    </View>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: "#0E1612" },
  header: { height: 58, paddingHorizontal: 16, flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  brand: { color: "#EDF4EE", fontSize: 19, fontWeight: "800", letterSpacing: -1 },
  actions: { flexDirection: "row", alignItems: "center", gap: 10 },
  menuButton: { width: 36, height: 36, alignItems: "center", justifyContent: "center", borderRadius: 18 },
  avatar: { width: 32, height: 32, borderRadius: 16, alignItems: "center", justifyContent: "center", backgroundColor: "#17291F" },
  avatarText: { color: "#7DF162", fontSize: 12, fontWeight: "800" },
  chatContent: { flex: 1, minHeight: 0, paddingHorizontal: 14, paddingTop: 14 },
  headingRow: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", paddingHorizontal: 4, paddingBottom: 14 },
  heading: { color: "#EDF4EE", fontSize: 21, fontWeight: "800" },
  meta: { color: "#89968D", fontSize: 9, fontWeight: "800", letterSpacing: 1.2 },
  searchBox: { height: 42, flexDirection: "row", alignItems: "center", gap: 8, paddingHorizontal: 12, borderRadius: 9, backgroundColor: "#161F1A" },
  searchIcon: { color: "#89968D", fontSize: 20 },
  searchInput: { flex: 1, color: "#EDF4EE", fontSize: 11, paddingVertical: 0 },
  listContent: { flexGrow: 1 },
  emptyList: { alignItems: "center", paddingTop: 34, gap: 8 },
  emptyIcon: { color: "#7DF162", fontSize: 29 },
  emptyTitle: { color: "#EDF4EE", fontSize: 12, fontWeight: "700" },
  emptyCopy: { maxWidth: 260, color: "#89968D", fontSize: 10, lineHeight: 16, textAlign: "center" },
  messagePreview: { alignItems: "center", paddingVertical: 18, paddingHorizontal: 14, gap: 8 },
  messageMark: { width: 48, height: 48, alignItems: "center", justifyContent: "center", borderRadius: 16, backgroundColor: "#7DF162" },
  messageMarkText: { color: "#061308", fontSize: 27, fontWeight: "900" },
  messageTitle: { color: "#EDF4EE", fontSize: 13, fontWeight: "700" },
  messageCopy: { maxWidth: 320, color: "#89968D", fontSize: 10, lineHeight: 16, textAlign: "center" },
  composer: { width: "100%", flexDirection: "row", gap: 8, marginTop: 8 },
  composerInput: { flex: 1, height: 40, paddingHorizontal: 12, borderRadius: 9, backgroundColor: "#161F1A", color: "#89968D", fontSize: 10 },
  sendButton: { width: 40, height: 40, alignItems: "center", justifyContent: "center", borderRadius: 20, backgroundColor: "#27352B", opacity: 0.4 },
  sendText: { color: "#7DF162", fontSize: 18, fontWeight: "800" },
  shade: { flex: 1, justifyContent: "flex-start", alignItems: "flex-end", paddingTop: 60, paddingHorizontal: 12, backgroundColor: "#00000066" },
  menuSheet: { width: 260, padding: 16, borderRadius: 12, backgroundColor: "#161F1A", gap: 9 },
  menuTitle: { color: "#EDF4EE", fontSize: 14, fontWeight: "800" },
  menuCopy: { color: "#89968D", fontSize: 10, lineHeight: 16 },
  menuAction: { paddingVertical: 10, paddingHorizontal: 11, borderRadius: 8, backgroundColor: "#0E1612" },
  menuActionText: { color: "#7DF162", fontSize: 11, fontWeight: "700" },
  closeAction: { alignSelf: "flex-end", paddingVertical: 6, paddingHorizontal: 8 },
  closeText: { color: "#89968D", fontSize: 10, fontWeight: "700" },
});
