FROM oven/bun:latest AS builder

WORKDIR /app

COPY package.json bun.lock* vite.config.ts ./

RUN bun install --frozen-lockfile

COPY . .

ENV NODE_ENV=production
RUN bun run build

FROM oven/bun:latest AS runner

RUN apt-get update && apt-get install -y --no-install-recommends curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

ENV NODE_ENV=production
ENV HOST=0.0.0.0
ENV PORT=3000

COPY --from=builder /app/build ./build
COPY --from=builder /app/node_modules ./node_modules
COPY --from=builder /app/prisma ./prisma
COPY --from=builder /app/prisma.config.ts ./
COPY --from=builder /app/package.json ./
COPY --from=builder /app/bun.lock* ./

RUN bun install --production --frozen-lockfile

EXPOSE 3000

CMD ["sh", "-c", "bunx prisma generate && bunx prisma migrate deploy && bun build/index.js"]
