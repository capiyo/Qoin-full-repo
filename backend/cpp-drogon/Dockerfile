FROM ubuntu:24.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install --yes --no-install-recommends \
    ca-certificates curl cmake make g++ libdrogon-dev libjsoncpp-dev libpq-dev libsqlite3-dev uuid-dev \
    libmariadb-dev libbrotli-dev libhiredis-dev libyaml-cpp-dev \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /workspace
COPY backend/cpp-drogon ./backend/cpp-drogon
RUN cmake -S backend/cpp-drogon -B build/drogon -DCMAKE_BUILD_TYPE=Release \
    && cmake --build build/drogon --parallel 2
RUN useradd --system --uid 10002 --create-home qoin
USER 10002:10002
EXPOSE 8080
ENTRYPOINT ["/workspace/build/drogon/qoin-gateway"]
