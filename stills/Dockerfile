# syntax=docker/dockerfile:1
# Service de visuels fixes : Remotion + Chrome headless.

FROM node:22-bookworm-slim
RUN corepack enable && \
    apt-get update && \
    apt-get install -y --no-install-recommends \
      ca-certificates fonts-liberation libnss3 libatk1.0-0 libatk-bridge2.0-0 \
      libcups2 libdrm2 libxkbcommon0 libxcomposite1 libxdamage1 libxfixes3 \
      libxrandr2 libgbm1 libasound2 libpango-1.0-0 libcairo2 && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /src
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY layout/package.json layout/package.json
COPY web/package.json web/package.json
COPY stills/package.json stills/package.json
COPY cli/package.json cli/package.json
RUN pnpm install --frozen-lockfile --filter @backline/stills...
COPY layout layout
COPY stills stills

# Le navigateur est telecharge a la construction : le premier visuel ne doit
# pas payer l'addition, et le conteneur n'a pas a sortir sur le reseau.
WORKDIR /src/stills
RUN node -e "import('@remotion/renderer').then((m) => m.ensureBrowser())"

ENV PORT=3000
EXPOSE 3000
CMD ["pnpm", "start"]
