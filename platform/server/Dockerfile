# Built from the repository root: the server compiles the stubs from
# contracts/proto/ and contracts/openapi/ and serves the web build, so the
# context has to include all three.

# The SPA is served by the server, so it is built into the jar rather than
# deployed as a second thing to keep in sync.
FROM node:24.21.0-slim AS web
RUN corepack enable
# The web install generates the API client from the bundled contract, so the
# contract is bundled first. Corepack reads the pinned pnpm version from the
# working directory, so each package is built from its own.
WORKDIR /src/contracts/openapi
COPY contracts/openapi ./
RUN pnpm install --frozen-lockfile && pnpm exec redocly bundle openapi.yaml --output ../openapi.yaml
WORKDIR /src/platform/web
COPY platform/web ./
RUN pnpm install --frozen-lockfile && pnpm build

FROM gradle:9.8.1-jdk25@sha256:e4f9e3d037cc0c7d8fc42cd97edb976dfd19a5b7314212b88b6de3807d13cd47 AS build
WORKDIR /src
COPY contracts/proto contracts/proto
COPY --from=web /src/contracts/openapi.yaml contracts/openapi.yaml
COPY platform/server platform/server
COPY --from=web /src/platform/web/app/dist platform/server/src/main/resources/static
RUN --mount=type=cache,target=/cache/gradle \
    GRADLE_USER_HOME=/cache/gradle gradle --no-daemon -p platform/server bootJar

FROM eclipse-temurin:25.0.4.1_1-jre@sha256:fcd7fd7b387f94bb2ac461478a7436ad8e349924c374ea8313919624dceae636
COPY --from=build /src/platform/server/build/libs/jjforge-server-*.jar /app/jjforge-server.jar
EXPOSE 8080
USER 65532
ENTRYPOINT ["java", "-jar", "/app/jjforge-server.jar"]
