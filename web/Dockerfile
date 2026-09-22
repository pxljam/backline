# syntax=docker/dockerfile:1
# Front React construit une fois, servi en statique.

FROM node:22-bookworm-slim AS build
RUN corepack enable
WORKDIR /src
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY layout/package.json layout/package.json
COPY web/package.json web/package.json
COPY stills/package.json stills/package.json
COPY cli/package.json cli/package.json
RUN pnpm install --frozen-lockfile --filter @backline/web...
COPY layout layout
COPY web web
RUN pnpm --filter @backline/web build

FROM caddy:2.8-alpine
COPY infra/web.Caddyfile /etc/caddy/Caddyfile
COPY --from=build /src/web/dist /srv
EXPOSE 80
