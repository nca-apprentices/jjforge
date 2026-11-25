# Built from the repository root: the server compiles the stubs from
# contracts/proto/ and contracts/openapi/, so the context has to include
# both.

FROM node:26.13.1-slim@sha256:e6bedef217677b276c591795e169840edd9096dbf13f29df8ef8d1856b8820bc AS contract
# Node 25 stopped bundling Corepack, so it comes from npm.
RUN npm install --global corepack@0.36.0 && corepack enable
# Bundles the contract that the server compiles its stubs from. Corepack
# reads the pinned pnpm version from the working directory.
WORKDIR /src/contracts/openapi
COPY contracts/openapi ./
RUN pnpm install --frozen-lockfile && pnpm exec redocly bundle openapi.yaml --output ../openapi.yaml

FROM gradle:9.8.2-jdk25@sha256:e46e15353757ef9c9be0197a243653e88d25ebcd4c3c841057cf839c11fcf898 AS build
WORKDIR /src
COPY contracts/proto contracts/proto
COPY --from=contract /src/contracts/openapi.yaml contracts/openapi.yaml
COPY platform/server platform/server
RUN --mount=type=cache,target=/cache/gradle \
    GRADLE_USER_HOME=/cache/gradle gradle --no-daemon -p platform/server bootJar

FROM eclipse-temurin:25.0.4.1_1-jre@sha256:fcd7fd7b387f94bb2ac461478a7436ad8e349924c374ea8313919624dceae636
COPY --from=build /src/platform/server/build/libs/jjforge-server-*.jar /app/jjforge-server.jar
EXPOSE 8080
USER 65532
ENTRYPOINT ["java", "-jar", "/app/jjforge-server.jar"]
