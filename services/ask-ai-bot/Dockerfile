# Install dependencies
FROM oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4 AS deps
WORKDIR /app
COPY services/ask-ai-bot/package.json services/ask-ai-bot/bun.lock ./
RUN bun install --frozen-lockfile

# Build stage
FROM oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4 AS build
WORKDIR /app
COPY --from=deps /app/node_modules ./node_modules
COPY services/ask-ai-bot/ ./
RUN bun run build

# Production stage
FROM oven/bun:1@sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4 AS production
WORKDIR /app
ENV NODE_ENV=production
ENV DOCS_PATH=/app/docs
ENV CODEBASE_PATH=/app/codebase

COPY --from=build /app/dist ./dist
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/package.json ./

# Copy documentation (only docs/ subdirectory with markdown files)
COPY documentation/docs ./docs

# Copy codebase source for code browsing
COPY ui/ ./codebase/ui/
COPY crates ./codebase/crates

# Empty index.ts for discraft start to detect
RUN touch index.ts

CMD ["bun", "run", "start"]
