const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

const eventId = `websocket-smoke-${Date.now()}`;
const socket = new WebSocket("ws://127.0.0.1:8080/api/v1/stream?symbols=WS_SMOKE");
const timeout = setTimeout(() => {
  socket.close();
  process.exitCode = 1;
  console.error("Timed out waiting for the market.update event through the Drogon WebSocket proxy");
}, 15000);

let injected = false;
socket.addEventListener("open", async () => {
  if (injected) return;
  injected = true;
  try {
    const response = await fetch("http://127.0.0.1:8081/internal/events", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        event_id: eventId,
        venue: "ci_ws",
        symbol: "WS_SMOKE",
        event_type: "trade",
        ts_ms: Date.now(),
        price: 100.25,
        size: 2,
        side: "buy"
      })
    });
    assert(response.ok, `Internal market ingestion failed: HTTP ${response.status} ${await response.text()}`);
  } catch (error) {
    clearTimeout(timeout);
    socket.close();
    console.error(error);
    process.exitCode = 1;
  }
});

socket.addEventListener("message", (message) => {
  let update;
  try {
    update = JSON.parse(String(message.data));
  } catch {
    return;
  }
  if (update.type !== "market.update" || update.event?.event_id !== eventId) return;
  try {
    assert(update.symbol === "WS_SMOKE", "Stream symbol did not match the subscribed instrument");
    assert(update.state?.symbol === "WS_SMOKE", "Stream omitted the persisted market state");
    assert(update.decision && typeof update.decision === "object", "Stream omitted the V1 decision mechanics");
    assert(Object.prototype.hasOwnProperty.call(update, "v1_shared_market_vector"), "Stream omitted the shared V1 market-vector result");
    assert(Object.prototype.hasOwnProperty.call(update, "v2_relationship_discovery"), "Stream omitted V2 discovery telemetry");
    clearTimeout(timeout);
    socket.close();
    console.log("WebSocket smoke passed: gateway proxy delivered the source event, state, V1 decision, shared vector and V2 discovery telemetry.");
  } catch (error) {
    clearTimeout(timeout);
    socket.close();
    console.error(error);
    process.exitCode = 1;
  }
});

socket.addEventListener("error", () => {
  clearTimeout(timeout);
  console.error("WebSocket connection failed");
  process.exitCode = 1;
});
