# Built from the repository root: the web app generates its API client from
# the contract in contracts/openapi, and the app itself is in platform/web,
# so the context has to include both.

FROM node:26.13.1-slim@sha256:e6bedef217677b276c591795e169840edd9096dbf13f29df8ef8d1856b8820bc AS build
# Node 25 stopped bundling Corepack, so it comes from npm.
RUN npm install --global corepack@0.36.0 && corepack enable
# The web install generates the API client from the bundled contract, so the
# contract is bundled first. Corepack reads the pinned pnpm version from the
# working directory, so each package is built from its own.
WORKDIR /src/contracts/openapi
COPY contracts/openapi ./
RUN pnpm install --frozen-lockfile && pnpm exec redocly bundle openapi.yaml --output ../openapi.yaml
WORKDIR /src/platform/web
COPY platform/web ./
RUN pnpm install --frozen-lockfile && pnpm build

FROM nginxinc/nginx-unprivileged:1.30.4-alpine@sha256:adf5042a17f4ecdd200c595fa9ffd1be37efb18f89a830bd1a00e4ab4d59d42c
COPY platform/web/app/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /src/platform/web/app/dist /usr/share/nginx/html
EXPOSE 8080
