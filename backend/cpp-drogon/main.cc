#include <drogon/drogon.h>
#include <drogon/WebSocketClient.h>
#include <drogon/WebSocketController.h>
#include <cstdlib>
#include <string>
#include <memory>
#include <mutex>
#include <unordered_map>
#include <vector>


namespace {
struct StreamProxySession {
    drogon::WebSocketClientPtr upstream_client;
    drogon::WebSocketConnectionPtr upstream_connection;
    std::weak_ptr<drogon::WebSocketConnection> downstream;
};
std::mutex streamSessionsMutex;
std::unordered_map<const drogon::WebSocketConnection*, std::shared_ptr<StreamProxySession>> streamSessions;
std::string upstreamWs() {
    const char* value = std::getenv("QOIN_AXUM_WS_URL");
    if (value && *value) return std::string(value);
    const char* hostport = std::getenv("QOIN_AXUM_HOSTPORT");
    return hostport && *hostport ? "ws://" + std::string(hostport) : "ws://127.0.0.1:8081";
}
}
class MarketStreamProxy : public drogon::WebSocketController<MarketStreamProxy> {
public:
    void handleNewMessage(const drogon::WebSocketConnectionPtr& connection,
                          std::string&& message,
                          const drogon::WebSocketMessageType&) override {
        std::shared_ptr<StreamProxySession> session;
        {
            std::lock_guard<std::mutex> lock(streamSessionsMutex);
            auto it = streamSessions.find(connection.get());
            if (it != streamSessions.end()) session = it->second;
        }
        if (session && session->upstream_connection && session->upstream_connection->connected()) {
            session->upstream_connection->send(message);
        }
    }
    void handleNewConnection(const drogon::HttpRequestPtr& request,
                              const drogon::WebSocketConnectionPtr& connection) override {
        auto session = std::make_shared<StreamProxySession>();
        session->downstream = connection;
        session->upstream_client = drogon::WebSocketClient::newWebSocketClient(upstreamWs());
        {
            std::lock_guard<std::mutex> lock(streamSessionsMutex);
            streamSessions[connection.get()] = session;
        }
        const auto symbol = request->getParameter("symbol");
        const auto symbols = request->getParameter("symbols");
        std::string path = "/internal/stream";
        if (!symbol.empty() || !symbols.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !symbols.empty()) path += "&";
            if (!symbols.empty()) path += "symbols=" + drogon::utils::urlEncode(symbols);
        }
        std::weak_ptr<drogon::WebSocketConnection> downstream = connection;
        auto client = session->upstream_client;
        client->setMessageHandler([downstream](std::string&& message,
            const drogon::WebSocketClientPtr&, const drogon::WebSocketMessageType&) {
            if (auto target = downstream.lock()) {
                if (target->connected()) target->send(message);
            }
        });
        client->setConnectionClosedHandler([downstream](const drogon::WebSocketClientPtr&) {
            if (auto target = downstream.lock()) target->forceClose();
        });
        auto upstreamRequest = drogon::HttpRequest::newHttpRequest();
        upstreamRequest->setPath(path);
        const auto authorization = request->getHeader("authorization");
        if (!authorization.empty()) upstreamRequest->addHeader("authorization", authorization);
        client->connectToServer(upstreamRequest,
            [downstream, client, key = connection.get()](drogon::ReqResult result,
                const drogon::HttpResponsePtr&, const drogon::WebSocketClientPtr&) {
                if (result != drogon::ReqResult::Ok) {
                    if (auto target = downstream.lock()) {
                        target->send("{\"type\":\"stream.error\",\"error\":\"upstream market stream unavailable\"}");
                        target->forceClose();
                    }
                    std::lock_guard<std::mutex> lock(streamSessionsMutex);
                    streamSessions.erase(key);
                    return;
                }
                std::lock_guard<std::mutex> lock(streamSessionsMutex);
                auto it = streamSessions.find(key);
                if (it != streamSessions.end()) it->second->upstream_connection = client->getConnection();
            });
    }
    void handleConnectionClosed(const drogon::WebSocketConnectionPtr& connection) override {
        std::shared_ptr<StreamProxySession> session;
        {
            std::lock_guard<std::mutex> lock(streamSessionsMutex);
            auto it = streamSessions.find(connection.get());
            if (it != streamSessions.end()) {
                session = it->second;
                streamSessions.erase(it);
            }
        }
        if (session) {
            if (session->upstream_connection) session->upstream_connection->forceClose();
            if (session->upstream_client) session->upstream_client->stop();
        }
    }
    WS_PATH_LIST_BEGIN
    WS_PATH_ADD("/api/v1/stream");
    WS_PATH_LIST_END
};

namespace {
std::string upstream() {
    const char* value = std::getenv("QOIN_AXUM_URL");
    if (value && *value) return std::string(value);
    const char* hostport = std::getenv("QOIN_AXUM_HOSTPORT");
    return hostport && *hostport ? "http://" + std::string(hostport) : "http://127.0.0.1:8081";
}
void proxyGet(const drogon::HttpRequestPtr& incoming,
              std::function<void(const drogon::HttpResponsePtr&)>&& callback,
              const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Get);
    request->setPath(path);
    request->addHeader("accept", "application/json");
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    // Privileged credentials are caller-supplied, never injected from gateway
    // environment variables into otherwise public proxy requests.
    const auto executionToken = incoming->getHeader("x-qoin-execution-token");
    if (!executionToken.empty()) request->addHeader("x-qoin-execution-token", executionToken);
    const auto ledgerMaintenanceToken = incoming->getHeader("x-qoin-ledger-maintenance-token");
    if (!ledgerMaintenanceToken.empty()) request->addHeader("x-qoin-ledger-maintenance-token", ledgerMaintenanceToken);
    client->sendRequest(request, [callback = std::move(callback)](drogon::ReqResult result,
        const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
            error->setStatusCode(drogon::k503ServiceUnavailable);
            callback(error);
            return;
        }
        auto forwarded = drogon::HttpResponse::newHttpResponse();
        forwarded->setStatusCode(response->getStatusCode());
        forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        forwarded->setBody(std::string(response->getBody()));
        callback(forwarded);
    });
}

void proxyPostJson(const drogon::HttpRequestPtr& incoming,
                   std::function<void(const drogon::HttpResponsePtr&)>&& callback,
                   const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Post);
    request->setPath(path);
    request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
    request->addHeader("accept", "application/json");
    request->setBody(std::string(incoming->getBody()));
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    // Privileged credentials are caller-supplied, never injected from gateway
    // environment variables into otherwise public proxy requests.
    const auto executionToken = incoming->getHeader("x-qoin-execution-token");
    if (!executionToken.empty()) request->addHeader("x-qoin-execution-token", executionToken);
    const auto ledgerMaintenanceToken = incoming->getHeader("x-qoin-ledger-maintenance-token");
    if (!ledgerMaintenanceToken.empty()) request->addHeader("x-qoin-ledger-maintenance-token", ledgerMaintenanceToken);
    client->sendRequest(request, [callback = std::move(callback)](
        drogon::ReqResult result, const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
            error->setStatusCode(drogon::k503ServiceUnavailable);
            callback(error);
            return;
        }
        auto forwarded = drogon::HttpResponse::newHttpResponse();
        forwarded->setStatusCode(response->getStatusCode());
        forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        forwarded->setBody(std::string(response->getBody()));
        callback(forwarded);
    });
}
}

void proxyDelete(const drogon::HttpRequestPtr& incoming,
                 std::function<void(const drogon::HttpResponsePtr&)>&& callback,
                 const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Delete);
    request->setPath(path);
    request->addHeader("accept", "application/json");
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    client->sendRequest(request, [callback = std::move(callback)](
        drogon::ReqResult result, const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
            error->setStatusCode(drogon::k503ServiceUnavailable);
            callback(error);
            return;
        }
        auto forwarded = drogon::HttpResponse::newHttpResponse();
        forwarded->setStatusCode(response->getStatusCode());
        forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        forwarded->setBody(std::string(response->getBody()));
        callback(forwarded);
    });
}

void proxyPutJson(const drogon::HttpRequestPtr& incoming,
                  std::function<void(const drogon::HttpResponsePtr&)>&& callback,
                  const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Put);
    request->setPath(path);
    request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
    request->addHeader("accept", "application/json");
    request->setBody(std::string(incoming->getBody()));
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    client->sendRequest(request, [callback = std::move(callback)](
        drogon::ReqResult result, const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
            error->setStatusCode(drogon::k503ServiceUnavailable);
            callback(error);
            return;
        }
        auto forwarded = drogon::HttpResponse::newHttpResponse();
        forwarded->setStatusCode(response->getStatusCode());
        forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        forwarded->setBody(std::string(response->getBody()));
        callback(forwarded);
    });
}

}

int main() {
    drogon::app().registerHandler("/v1/signal", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/v1/signal");
    }, {drogon::Post});
    drogon::app().registerHandler("/health", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) { proxyGet(req, std::move(cb), "/health"); }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/state", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        proxyGet(req, std::move(cb), "/internal/state" + (symbol.empty() ? "" : "?symbol=" + drogon::utils::urlEncode(symbol)));
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/vector/history", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto limit = req->getParameter("limit");
        const auto atMs = req->getParameter("at_ms");
        std::string path = "/internal/vector/history";
        if (!symbol.empty() || !limit.empty() || !atMs.empty()) {
            path += "?";
            bool hasParameter = false;
            if (!symbol.empty()) {
                path += "symbol=" + drogon::utils::urlEncode(symbol);
                hasParameter = true;
            }
            if (!limit.empty()) {
                if (hasParameter) path += "&";
                path += "limit=" + drogon::utils::urlEncode(limit);
                hasParameter = true;
            }
            if (!atMs.empty()) {
                if (hasParameter) path += "&";
                path += "at_ms=" + drogon::utils::urlEncode(atMs);
            }
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/set-ledger", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto limit = req->getParameter("limit");
        std::string path = "/internal/set-ledger";
        if (!symbol.empty() || !limit.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !limit.empty()) path += "&";
            if (!limit.empty()) path += "limit=" + drogon::utils::urlEncode(limit);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/set-ledger/reconstruct", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto limit = req->getParameter("limit");
        std::string path = "/internal/set-ledger/reconstruct";
        if (!symbol.empty() || !limit.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !limit.empty()) path += "&";
            if (!limit.empty()) path += "limit=" + drogon::utils::urlEncode(limit);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/set-ledger/backfill", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto names = {"symbol", "after_ts_ms", "after_event_id", "after_venue", "batch_size"};
        std::string path = "/internal/set-ledger/backfill";
        bool first = true;
        for (const auto* name : names) {
            const auto value = req->getParameter(name);
            if (value.empty()) continue;
            path += first ? "?" : "&";
            first = false;
            path += std::string(name) + "=" + drogon::utils::urlEncode(value);
        }
        proxyPostJson(req, std::move(cb), path);
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/set-ledger/corrections", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto names = {"symbol", "status", "limit"};
        std::string path = "/internal/set-ledger/corrections";
        bool first = true;
        for (const auto* name : names) {
            const auto value = req->getParameter(name);
            if (value.empty()) continue;
            path += first ? "?" : "&";
            first = false;
            path += std::string(name) + "=" + drogon::utils::urlEncode(value);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/set-ledger/corrections", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/set-ledger/corrections");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/set-ledger/integrity", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto limit = req->getParameter("limit");
        std::string path = "/internal/set-ledger/integrity";
        if (!symbol.empty() || !limit.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !limit.empty()) path += "&";
            if (!limit.empty()) path += "limit=" + drogon::utils::urlEncode(limit);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/backtest", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto names = {"symbol", "venue", "from_ts_ms", "to_ts_ms", "limit", "initial_capital", "fee_bps", "slippage_bps", "latency_ms", "max_participation_rate", "market_impact_bps", "algorithm_id", "algorithm_version"};
        std::string path = "/internal/backtest";
        bool first = true;
        for (const auto* name : names) {
            const auto value = req->getParameter(name);
            if (value.empty()) continue;
            path += first ? "?" : "&";
            first = false;
            path += std::string(name) + "=" + drogon::utils::urlEncode(value);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});



    drogon::app().registerHandler("/api/v1/operating-cycle", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Put) proxyPutJson(req, std::move(cb), "/internal/operating-cycle");
        else proxyGet(req, std::move(cb), "/internal/operating-cycle");
    }, {drogon::Get, drogon::Put});
    drogon::app().registerHandler("/api/v1/operating-cycle/start", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/operating-cycle/start");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/operating-cycle/stop", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/operating-cycle/stop");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/multi-horizon", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/multi-horizon");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/market-horizons", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        std::string path = "/internal/market-horizons";
        const auto symbol = req->getParameter("symbol");
        const auto venue = req->getParameter("venue_id");
        const auto minEvents = req->getParameter("min_events");
        std::vector<std::string> query;
        if (!symbol.empty()) query.push_back("symbol=" + drogon::utils::urlEncode(symbol));
        if (!venue.empty()) query.push_back("venue_id=" + drogon::utils::urlEncode(venue));
        if (!minEvents.empty()) query.push_back("min_events=" + drogon::utils::urlEncode(minEvents));
        for (const auto& item : query) path += (path.find('?') == std::string::npos ? "?" : "&") + item;
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/investment-plan", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Put) proxyPutJson(req, std::move(cb), "/internal/investment-plan");
        else proxyGet(req, std::move(cb), "/internal/investment-plan");
    }, {drogon::Get, drogon::Put});
    drogon::app().registerHandler("/api/v1/paper-session", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/paper-session");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/paper-session/history", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        std::string path = "/internal/paper-session/history";
        const auto symbol = req->getParameter("symbol");
        const auto cycle = req->getParameter("cycle_id");
        const auto limit = req->getParameter("limit");
        std::vector<std::string> query;
        if (!symbol.empty()) query.push_back("symbol=" + drogon::utils::urlEncode(symbol));
        if (!cycle.empty()) query.push_back("cycle_id=" + drogon::utils::urlEncode(cycle));
        if (!limit.empty()) query.push_back("limit=" + drogon::utils::urlEncode(limit));
        for (const auto& item : query) path += (path.find('?') == std::string::npos ? "?" : "&") + item;
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/paper-session/runs", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto limit = req->getParameter("limit");
        proxyGet(req, std::move(cb), "/internal/paper-session/runs" + (limit.empty() ? std::string{} : "?limit=" + drogon::utils::urlEncode(limit)));
    }, {drogon::Get});

    drogon::app().registerHandler("/api/v1/me", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/users/me");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/me/algorithm-selections", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Get) {
            const auto symbol = req->getParameter("symbol");
            proxyGet(req, std::move(cb), "/internal/me/algorithm-selections" +
                (symbol.empty() ? std::string{} : "?symbol=" + drogon::utils::urlEncode(symbol)));
        } else proxyPostJson(req, std::move(cb), "/internal/me/algorithm-selections");
    }, {drogon::Get, drogon::Post});
    drogon::app().registerHandler("/api/v1/me/algorithm-decision", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        proxyGet(req, std::move(cb), "/internal/me/algorithm-decision" +
            (symbol.empty() ? std::string{} : "?symbol=" + drogon::utils::urlEncode(symbol)));
    }, {drogon::Get});
    // Broker credentials are provisioned only through backend deployment secrets.
    drogon::app().registerHandler("/api/v1/market-vector", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/market-vector");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/market-vector/history", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/market-vector/history");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/notifications", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/notifications");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/notifications/unread-summary", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/notifications/unread-summary");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/notifications/record", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/notifications/record");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/notifications/register-token", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/notifications/register-token");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/notifications/mark-read", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyPostJson(req, std::move(cb), "/internal/notifications/mark-read");
    }, {drogon::Post});
    drogon::app().registerHandler("/api/v1/notifications/preferences", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::HttpMethod::Get) proxyGet(req, std::move(cb), "/internal/notifications/preferences");
        else proxyPostJson(req, std::move(cb), "/internal/notifications/preferences");
    }, {drogon::Get, drogon::Post});
    drogon::app().registerHandler("/api/v1/brokers/configuration", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/brokers/configuration");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/broker-accounts", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/broker-accounts");
    }, {drogon::Get});

    // Live order submission and reconciliation remain private Axum operations.
    drogon::app().registerHandler("/api/v1/trading/readiness", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto brokers = req->getParameter("brokers");
        const auto path = "/internal/trading/readiness" +
            (brokers.empty() ? std::string{} : "?brokers=" + drogon::utils::urlEncode(brokers));
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/brokers", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/brokers");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/algorithms", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/algorithms");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/instruments", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/instruments");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/calendar", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/calendar");
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/news", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto category = req->getParameter("category");
        proxyGet(req, std::move(cb), "/internal/news" + (category.empty() ? "" : "?category=" + drogon::utils::urlEncode(category)));
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/vector", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        proxyGet(req, std::move(cb), "/internal/vector" + (symbol.empty() ? "" : "?symbol=" + drogon::utils::urlEncode(symbol)));
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/events", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Get) {
            const auto symbol = req->getParameter("symbol");
            const auto limit = req->getParameter("limit");
            std::string path = "/internal/events";
            if (!symbol.empty() || !limit.empty()) {
                path += "?";
                if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
                if (!symbol.empty() && !limit.empty()) path += "&";
                if (!limit.empty()) path += "limit=" + drogon::utils::urlEncode(limit);
            }
            proxyGet(req, std::move(cb), path);
            return;
        }
        auto client = drogon::HttpClient::newHttpClient(upstream());
        auto request = drogon::HttpRequest::newHttpRequest();
        request->setMethod(drogon::Post);
        request->setPath("/internal/events");
        request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        request->setBody(std::string(req->getBody()));
        client->sendRequest(request, [cb = std::move(cb)](drogon::ReqResult result, const drogon::HttpResponsePtr& response) mutable {
            if (result != drogon::ReqResult::Ok || !response) {
                auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
                error->setStatusCode(drogon::k503ServiceUnavailable);
                cb(error);
                return;
            }
            auto forwarded = drogon::HttpResponse::newHttpResponse();
            forwarded->setStatusCode(response->getStatusCode());
            forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
            forwarded->setBody(std::string(response->getBody()));
            cb(forwarded);
        });
    }, {drogon::Get}); // Market ingestion is provider/internal-only; never expose public writes.
    drogon::app().registerHandler("/api/v1/strategy", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Get) {
            proxyGet(req, std::move(cb), "/internal/strategy");
            return;
        }
        auto client = drogon::HttpClient::newHttpClient(upstream());
        auto request = drogon::HttpRequest::newHttpRequest();
        request->setMethod(drogon::Put);
        request->setPath("/internal/strategy");
        request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        request->setBody(std::string(req->getBody()));
        client->sendRequest(request, [cb = std::move(cb)](drogon::ReqResult result, const drogon::HttpResponsePtr& response) mutable {
            if (result != drogon::ReqResult::Ok || !response) {
                auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "flow engine unavailable"}});
                error->setStatusCode(drogon::k503ServiceUnavailable);
                cb(error);
                return;
            }
            auto forwarded = drogon::HttpResponse::newHttpResponse();
            forwarded->setStatusCode(response->getStatusCode());
            forwarded->setContentTypeCode(drogon::CT_APPLICATION_JSON);
            forwarded->setBody(std::string(response->getBody()));
            cb(forwarded);
        });
    }, {drogon::Get}); // Strategy mutation is an operator-only operation, not a public API.
    const char* portEnv = std::getenv("QOIN_DROGON_PORT");
    const auto port = portEnv ? std::stoi(portEnv) : 8080;
    const char* hostEnv = std::getenv("QOIN_DROGON_HOST");
    const std::string host = hostEnv ? std::string(hostEnv) : "127.0.0.1";
    drogon::app().addListener(host, static_cast<uint16_t>(port));
    drogon::app().setLogLevel(trantor::Logger::kInfo);
    LOG_INFO << "Qoin gateway listening on " << host << ":" << port << "; upstream=" << upstream();
    drogon::app().run();
}
