# Backline

Management tool for artistic collectives. **The full specification is in [PRD.md](PRD.md) (written in French) — read it before doing anything.**

- Status: §17 steps 1 to 9 implemented. Rust API, React front-end, `layout/` (shared render engine), `stills/`, `cli/`, Telegram bot, complete Docker Compose.
- Nothing is committed or deployed yet. Manual check of the studio in a browser still to do.
- Stack: Rust (Axum, sqlx) · React + Vite + TypeScript + Tailwind · PostgreSQL · Remotion · Typst · Docker Compose · mise
- **Source code in English** — comments, identifiers, test names, logs, CLI output.
- **User-facing copy stays in French**: web UI strings, Telegram bot messages, `AppError` messages (they surface verbatim in the interface), notification titles and bodies, the tech-rider PDF, seed data. See §21 for the glossary.
- `mise run check` before every push. No CI. Manual push and deploy.
