import { useState } from "react";

type Conversation = { id: string; name: string; preview: string; time: string; unread?: number };

const conversations: Conversation[] = [];

export function ChatWorkspace() {
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const visible = conversations.filter((item) =>
    item.name.toLowerCase().includes(query.toLowerCase()) ||
    item.preview.toLowerCase().includes(query.toLowerCase())
  );
  const selected = conversations.find((item) => item.id === selectedId) ?? null;

  return (
    <section className="qoin-chat-workspace" aria-label="Qoin conversations">
      <aside className="qoin-chat-list">
        <div className="qoin-chat-list-heading">
          <strong>Chats</strong>
          <span>MESSAGES</span>
        </div>
        <label className="qoin-chat-search">
          <span aria-hidden="true">⌕</span>
          <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search chats" aria-label="Search chats" />
        </label>
        <div className="qoin-chat-list-body">
          {visible.length ? visible.map((conversation) => (
            <button key={conversation.id} type="button" className={selectedId === conversation.id ? "qoin-chat-row active" : "qoin-chat-row"} onClick={() => setSelectedId(conversation.id)}>
              <span className="qoin-chat-avatar">{conversation.name.slice(0, 1).toUpperCase()}</span>
              <span className="qoin-chat-row-copy"><strong>{conversation.name}</strong><small>{conversation.preview}</small></span>
              <span className="qoin-chat-row-meta"><small>{conversation.time}</small>{conversation.unread ? <i>{conversation.unread}</i> : null}</span>
            </button>
          )) : (
            <div className="qoin-chat-empty-list">
              <span className="qoin-chat-empty-icon" aria-hidden="true">◌</span>
              <strong>{query ? "No matching chats" : "No conversations yet"}</strong>
              <p>{query ? "Try another name or keyword." : "Your conversations will appear here when Qoin messaging is connected."}</p>
            </div>
          )}
        </div>
      </aside>
      <section className="qoin-message-panel" aria-label="Message conversation">
        {selected ? (
          <>
            <header className="qoin-message-header">
              <span className="qoin-chat-avatar">{selected.name.slice(0, 1).toUpperCase()}</span>
              <div><strong>{selected.name}</strong><small>Conversation</small></div>
            </header>
            <div className="qoin-message-empty">
              <strong>Messages are not synced yet</strong>
              <p>This workspace is ready for the Qoin messaging service. No messages are fabricated or stored only in this browser.</p>
            </div>
            <form className="qoin-message-composer" onSubmit={(event) => event.preventDefault()}>
              <input aria-label="Message" placeholder="Messaging service not connected" disabled />
              <button type="submit" disabled aria-label="Send message">↑</button>
            </form>
          </>
        ) : (
          <div className="qoin-message-welcome">
            <span className="qoin-message-mark" aria-hidden="true">q</span>
            <strong>Your messages</strong>
            <p>Select a conversation to open it. Your most recent chat will appear here once messaging is connected.</p>
          </div>
        )}
      </section>
    </section>
  );
}
