# ── Stage 1: build ───────────────────────────────────────────────────────────
FROM node:22-slim AS builder
WORKDIR /app

COPY package.json ./
RUN npm install

COPY . .
RUN npm run build

# ── Stage 2: runtime ─────────────────────────────────────────────────────────
FROM node:22-slim AS runtime

# latexmk + xelatex para compilação de LaTeX via /api/latex/compile
RUN apt-get update && apt-get install -y --no-install-recommends \
      latexmk \
      texlive-xetex \
      texlive-latex-extra \
      texlive-fonts-recommended \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/.output .output

ENV NODE_ENV=production
EXPOSE 3000
CMD ["node", ".output/server/index.mjs"]
