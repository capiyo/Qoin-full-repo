#include <drogon/drogon.h>
#include <drogon/WebSocketClient.h>
#include <drogon/WebSocketController.h>
#include <cstdlib>
#include <string>
#include <memory>
#include <mutex>
#include <unordered_map>


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
    return value ? std::string(value) : "ws://127.0.0.1:8081";
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
    return value ? std::string(value) : "http://127.0.0.1:8081";
}
void proxyGet(const drogon::HttpRequestPtr&,
              std::function<void(const drogon::HttpResponsePtr&)>&& callback,
              const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Get);
    request->setPath(path);
    request->addHeader("accept", "application/json");
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
}
int main() {
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
        std::string path = "/internal/vector/history";
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
        const auto names = {"symbol", "from_ts_ms", "to_ts_ms", "limit", "initial_capital", "fee_bps", "slippage_bps"};
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
    }, {drogon::Get, drogon::Post});
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
    }, {drogon::Get, drogon::Put});
    const char* portEnv = std::getenv("QOIN_DROGON_PORT");
    const auto port = portEnv ? std::stoi(portEnv) : 8080;
    drogon::app().addListener("127.0.0.1", static_cast<uint16_t>(port));
    drogon::app().setLogLevel(trantor::Logger::kInfo);
    LOG_INFO << "Qoin gateway listening on loopback port " << port << "; upstream=" << upstream();
    drogon::app().run();
}
