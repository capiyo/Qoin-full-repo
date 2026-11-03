#include <drogon/drogon.h>
#include <cstdlib>
#include <string>

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
        const auto assetClass = req->getParameter("asset_class");
        std::string path = "/internal/state";
        if (!symbol.empty() || !assetClass.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !assetClass.empty()) path += "&";
            if (!assetClass.empty()) path += "asset_class=" + drogon::utils::urlEncode(assetClass);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/instruments", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto assetClass = req->getParameter("asset_class");
        const std::string path = "/internal/instruments?asset_class=" + drogon::utils::urlEncode(assetClass);
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/market-horizons", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto minEvents = req->getParameter("min_events");
        std::string path = "/internal/market-horizons";
        if (!symbol.empty() || !minEvents.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !minEvents.empty()) path += "&";
            if (!minEvents.empty()) path += "min_events=" + drogon::utils::urlEncode(minEvents);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/vector", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        const auto symbol = req->getParameter("symbol");
        const auto assetClass = req->getParameter("asset_class");
        std::string path = "/internal/vector";
        if (!symbol.empty() || !assetClass.empty()) {
            path += "?";
            if (!symbol.empty()) path += "symbol=" + drogon::utils::urlEncode(symbol);
            if (!symbol.empty() && !assetClass.empty()) path += "&";
            if (!assetClass.empty()) path += "asset_class=" + drogon::utils::urlEncode(assetClass);
        }
        proxyGet(req, std::move(cb), path);
    }, {drogon::Get});
    drogon::app().registerHandler("/api/v1/multi-horizon", [](const drogon::HttpRequestPtr& req,
        std::function<void(const drogon::HttpResponsePtr&)>&& cb) {
        auto client = drogon::HttpClient::newHttpClient(upstream());
        auto request = drogon::HttpRequest::newHttpRequest();
        request->setMethod(drogon::Post);
        request->setPath("/internal/multi-horizon");
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
