# SPDX-License-Identifier: Apache-2.0
FROM node:24-alpine AS base
RUN corepack enable
WORKDIR /app

FROM base AS deps
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile

FROM base AS build
# next.config bakes the /api and /hubs proxy target into the build — standalone rewrites are
# resolved at BUILD time, not runtime — so BACKEND_ORIGIN must be present here, not just at runtime.
ARG BACKEND_ORIGIN=http://bot-shard-0:5080
ENV BACKEND_ORIGIN=$BACKEND_ORIGIN
COPY --from=deps /app/node_modules ./node_modules
COPY . .
# Invoke next directly to avoid pnpm's pre-run dependency re-check.
RUN node node_modules/next/dist/bin/next build

FROM node:24-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV PORT=3000
# Bind all interfaces; without this Next's standalone server can bind loopback and
# published ports won't reach it.
ENV HOSTNAME="0.0.0.0"
COPY --from=build /app/public ./public
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static ./.next/static
EXPOSE 3000
CMD ["node", "server.js"]
