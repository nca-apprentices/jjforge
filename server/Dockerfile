# Built from the repository root: the server compiles the stubs from proto/ and openapi.yaml
# and serves the web build, so the context has to include all three.

# The SPA is served by the server, so it is built into the jar rather than
# deployed as a second thing to keep in sync.
FROM node:24-slim AS web
WORKDIR /src/web
RUN corepack enable
COPY web/package.json web/pnpm-lock.yaml web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY web ./
RUN pnpm build

FROM gradle:9-jdk25 AS build
WORKDIR /src
COPY proto proto
COPY openapi.yaml openapi.yaml
COPY server server
COPY --from=web /src/web/dist server/src/main/resources/static
RUN --mount=type=cache,target=/cache/gradle \
    GRADLE_USER_HOME=/cache/gradle gradle --no-daemon -p server bootJar

FROM eclipse-temurin:25-jre
COPY --from=build /src/server/build/libs/jjforge-server-*.jar /app/jjforge-server.jar
EXPOSE 8080
USER 65532
ENTRYPOINT ["java", "-jar", "/app/jjforge-server.jar"]
