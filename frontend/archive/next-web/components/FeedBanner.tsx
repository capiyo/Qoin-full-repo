import type { FeedStatus } from "@qoin/core";

// Prominent, dismiss-proof banner for degraded feed states — separate from the
// small StatusBadge, which stays visible even when everything is fine.
export function FeedBanner({ status }: { status: FeedStatus }) {
  if (status === "live" || status === "connecting") return null;

  const isStale = status === "stale";
  return (
    <div
      role="status"
      className={`mb-4 rounded-lg px-4 py-2.5 text-sm font-medium flex items-center gap-2 ${
        isStale
          ? "bg-qoin-warning/10 text-qoin-warning border border-qoin-warning/30"
          : "bg-qoin-bearish/10 text-qoin-bearish border border-qoin-bearish/30"
      }`}
    >
      <span className="h-1.5 w-1.5 rounded-full bg-current shrink-0" />
      {isStale
        ? "Price data may be delayed — last update over 5s ago. Figures shown are not guaranteed current."
        : "Disconnected from the live feed. Reconnecting…"}
    </div>
  );
}
