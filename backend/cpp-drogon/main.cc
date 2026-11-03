#include <drogon/drogon.h>
#include <cstdlib>
#include <string>

namespace {
std::string upstream() {
    const char* value = std::getenv("QOIN_AXUM_URL");
    return value ? std::string(value) : "http://127.0.0.1:8081";
}
bool isAllowedOrigin(const std::string& origin) {
    const char* configured = std::getenv("QOIN_WEB_ORIGINS");
    if (!configured || !*configured) {
        return origin == "http://localhost:5173" || origin == "http://127.0.0.1:5173";
    }
    const std::string origins(configured);
    std::size_t start = 0;
    while (start <= origins.size()) {
        const auto end = origins.find(',', start);
        const auto length = end == std::string::npos ? std::string::npos : end - start;
        const auto raw = origins.substr(start, length);
        const auto first = raw.find_first_not_of(" \t");
        const auto last = raw.find_last_not_of(" \t");
        if (first != std::string::npos && raw.substr(first, last - first + 1) == origin) {
            return true;
        }
        if (end == std::string::npos) break;
        start = end + 1;
    }
    return false;
}
void corsOptions(const drogon::HttpRequestPtr&,
                 std::function<void(const drogon::HttpResponsePtr&)>&& callback) {
    auto response = drogon::HttpResponse::newHttpResponse();
    response->setStatusCode(drogon::k204NoContent);
    callback(response);
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
void proxyResearchPost(const drogon::HttpRequestPtr& incoming,
                        std::function<void(const drogon::HttpResponsePtr&)>&& callback) {
    const char* configured = std::getenv("QOIN_RESEARCH_API_URL");
    const std::string target = configured ? std::string(configured) : "http://127.0.0.1:8082";
    auto client = drogon::HttpClient::newHttpClient(target);
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(drogon::Post);
    request->setPath("/v1/signal");
    request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
    request->addHeader("accept", "application/json");
    request->setBody(std::string(incoming->getBody()));
    client->sendRequest(request, [callback = std::move(callback)](drogon::ReqResult result,
        const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "research signal service unavailable"}});
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
void proxyResearchPortfolio(const drogon::HttpRequestPtr& incoming,
                              std::function<void(const drogon::HttpResponsePtr&)>&& callback) {
    const char* configured = std::getenv("QOIN_RESEARCH_API_URL");
    const std::string target = configured ? std::string(configured) : "http://127.0.0.1:8082";
    auto client = drogon::HttpClient::newHttpClient(target);
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(incoming->method());
    request->setPath("/v1/portfolio");
    request->addHeader("accept", "application/json");
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    if (incoming->method() == drogon::Put) {
        request->setContentTypeCode(drogon::CT_APPLICATION_JSON);
        request->setBody(std::string(incoming->getBody()));
    }
    client->sendRequest(request, [callback = std::move(callback)](drogon::ReqResult result,
        const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "portfolio service unavailable"}});
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
int main() {
    // Apply CORS after Drogon has generated the response, including its automatic
    // OPTIONS responses. Restrict browser origins; this API uses bearer tokens,
    // not cookie credentials.
    drogon::app().registerPreSendingAdvice([](const drogon::HttpRequestPtr& request,
                                               const drogon::HttpResponsePtr& response) {
        response->removeHeader("Access-Control-Allow-Origin");
        response->removeHeader("Access-Control-Allow-Methods");
        response->removeHeader("Access-Control-Allow-Headers");
        response->removeHeader("Access-Control-Max-Age");
        const auto origin = request->getHeader("origin");
        if (origin.empty() || !isAllowedOrigin(origin)) return;
        response->addHeader("Access-Control-Allow-Origin", origin);
        response->addHeader("Access-Control-Allow-Methods", "GET, POST, PUT, OPTIONS");
        response->addHeader("Access-Control-Allow-Headers", "Authorization, Content-Type");
        response->addHeader("Access-Control-Max-Age", "600");
        response->addHeader("Vary", "Origin");
    });
    drogon::app().registerHandler("/v1/portfolio", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Options) {
            corsOptions(req, std::move(cb));
            return;
        }
        proxyResearchPortfolio(req, std::move(cb));
    }, {drogon::Get, drogon::Put, drogon::Options});

    drogon::app().registerHandler("/v1/signal", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Options) {
            corsOptions(req, std::move(cb));
            return;
        }
        proxyResearchPost(req, std::move(cb));
    }, {drogon::Post, drogon::Options});

    drogon::app().registerHandler("/health", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        if (req->method() == drogon::Options) {
            corsOptions(req, std::move(cb));
            return;
        }
        proxyGet(req, std::move(cb), "/health");
    }, {drogon::Get, drogon::Options});
    drogon::app().registerHandler("/api/v1/state", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        proxyGet(req, std::move(cb), "/internal/state" + (symbol.empty() ? "" : "?symbol=" + drogon::utils::urlEncode(symbol)));
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
    }, {drogon::Post});
    // Strategy mutation is intentionally not exposed through the public gateway.
    // Keep administrative writes on the loopback-only Axum service until an
    // authenticated operator identity and authorization policy are implemented.
    drogon::app().registerHandler("/api/v1/strategy", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyGet(req, std::move(cb), "/internal/strategy");
    }, {drogon::Get});
    const char* portEnv = std::getenv("QOIN_DROGON_PORT");
    const auto port = portEnv ? std::stoi(portEnv) : 8080;
    drogon::app().addListener("127.0.0.1", static_cast<uint16_t>(port));
    drogon::app().setLogLevel(trantor::Logger::kInfo);
    LOG_INFO << "Qoin gateway listening on loopback port " << port << "; upstream=" << upstream();
    drogon::app().run();
}
