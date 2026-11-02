#include <drogon/drogon.h>
#include <cstdlib>
#include <string>
#include <vector>

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

void proxyRequest(const drogon::HttpRequestPtr& incoming,
                  std::function<void(const drogon::HttpResponsePtr&)>&& callback,
                  const std::string& path) {
    auto client = drogon::HttpClient::newHttpClient(upstream());
    auto request = drogon::HttpRequest::newHttpRequest();
    request->setMethod(incoming->method());
    request->setPath(path);
    request->setBody(std::string(incoming->body()));
    request->setContentTypeCode(incoming->getContentType());
    const auto authorization = incoming->getHeader("authorization");
    if (!authorization.empty()) request->addHeader("authorization", authorization);
    const auto internalKey = incoming->getHeader("x-qoin-internal-key");
    if (!internalKey.empty()) request->addHeader("x-qoin-internal-key", internalKey);
    client->sendRequest(request, [callback = std::move(callback)](drogon::ReqResult result,
        const drogon::HttpResponsePtr& response) mutable {
        if (result != drogon::ReqResult::Ok || !response) {
            auto error = drogon::HttpResponse::newHttpJsonResponse(Json::Value{{"error", "Qoin API unavailable"}});
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

    const std::vector<std::pair<std::string, drogon::HttpMethod>> authRoutes = {
        {"/api/auth/firebase-login", drogon::Post}, {"/api/auth/register", drogon::Post},
        {"/api/auth/pin-login", drogon::Post}, {"/api/auth/set-pin", drogon::Post},
        {"/api/auth/me", drogon::Get}, {"/api/notifications/register-token", drogon::Post},
        {"/api/notifications", drogon::Get}, {"/api/notifications/unread-summary", drogon::Get},
        {"/api/notifications/mark-read", drogon::Put}, {"/api/notifications/preferences", drogon::Get},
        {"/api/notifications/preferences", drogon::Put}
    };
    for (const auto& route : authRoutes) {
        const auto path = route.first;
        const auto method = route.second;
        drogon::app().registerHandler(path, [path](const drogon::HttpRequestPtr& req,
            std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
            proxyRequest(req, std::move(cb), path);
        }, {method});
    }
    drogon::app().registerHandler("/api/auth/check-user/{1}", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        proxyRequest(req, std::move(cb), req->path());
    }, {drogon::Get});
\n    drogon::app().registerHandler("/health", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) { proxyGet(req, std::move(cb), "/health"); }, {drogon::Get});
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
