# Production image. Build & run:
#   docker compose --profile app up -d --build
# Migrations run by the one-shot `migrate` compose service (build stage has
# the full Prisma CLI); the app container only runs the standalone server.
#
# Does NOT touch the Rust/WASM toolchain — web/lib/wasm/{crochet_wasm.js,
# crochet_wasm_bg.wasm,*.d.ts} are wasm-bindgen output, committed to git
# (see ../README.md's rebuild command). If core/ or wasm/ changed, rebuild
# and commit those files before building this image.

FROM node:22-alpine AS deps
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci

FROM node:22-alpine AS build
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY . .
RUN npx prisma generate && npm run build

FROM node:22-alpine AS runtime
WORKDIR /app
ENV NODE_ENV=production
ENV HOSTNAME=0.0.0.0
ENV PORT=3000

# Next standalone server (self-contained node_modules) + static assets.
# The WASM binary is served from lib/wasm/ as a build-time-resolved static
# asset (see lib/wasm/index.ts's comment on `new URL(...,
# import.meta.url)`) — Next's file-tracing follows that reference into
# .next/standalone automatically, same as any other imported asset.
COPY --from=build /app/.next/standalone ./
COPY --from=build /app/.next/static ./.next/static
COPY --from=build /app/public ./public

EXPOSE 3000
CMD ["node", "server.js"]
