# Built from the repository root: the server compiles the stubs from shared/proto/
# and shared/api/ and serves the web build, so the context has to include all
# three.

# The SPA is served by the server, so it is built into the jar rather than
# deployed as a second thing to keep in sync.
FROM node:24.21.0-slim AS web
RUN corepack enable
# The web install generates the API client from the compiled contract, so the
# contract is compiled first. Corepack reads the pinned pnpm version from the
# working directory, so each package is built from its own.
WORKDIR /src/shared/api
COPY shared/api ./
RUN pnpm install --frozen-lockfile && pnpm exec tsp compile .
WORKDIR /src/web
COPY web ./
RUN pnpm install --frozen-lockfile && pnpm build

FROM gradle:9.8.0-jdk25@sha256:7086a4cd10d568b35cafd6d5d30323865f3ce1f23ccddc9570ff3e3c9c8cd6e9 AS build
WORKDIR /src
COPY shared/proto shared/proto
COPY --from=web /src/shared/openapi.yaml shared/openapi.yaml
COPY jvm jvm
COPY --from=web /src/web/apps/web/dist jvm/src/main/resources/static
RUN --mount=type=cache,target=/cache/gradle \
    GRADLE_USER_HOME=/cache/gradle gradle --no-daemon -p jvm bootJar

FROM eclipse-temurin:25.0.4_7-jre@sha256:bb036ed6cfdc57e3da7c22634d15f1b840d2caf76183861c80e81ca4b5104abb
COPY --from=build /src/jvm/build/libs/jjforge-server-*.jar /app/jjforge-server.jar
EXPOSE 8080
USER 65532
ENTRYPOINT ["java", "-jar", "/app/jjforge-server.jar"]
