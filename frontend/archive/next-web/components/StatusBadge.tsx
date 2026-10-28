import type { FeedStatus } from "@qoin/core";

const CONFIG: Record<FeedStatus, { label: string; dot: string; text: string }> = {
  connecting: { label: "Connecting", dot: "bg-qoin-warning", text: "text-qoin-warning" },
  live: { label: "Live", dot: "bg-qoin-bullish", text: "text-qoin-bullish" },
  stale: { label: "Stale data", dot: "bg-qoin-warning", text: "text-qoin-warning" },
  disconnected: { label: "Disconnected", dot: "bg-qoin-bearish", text: "text-qoin-bearish" },
};

export function StatusBadge({ status }: { status: FeedStatus }) {
  const c = CONFIG[status];
  return (
    <span className={`inline-flex items-center gap-1.5 text-xs font-medium ${c.text}`}>
      <span className={`h-1.5 w-1.5 rounded-full ${c.dot} ${status === "live" ? "animate-pulse" : ""}`} />
      {c.label}
    </span>
  );
}
