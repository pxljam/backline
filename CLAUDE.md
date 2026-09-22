# Backline

Outil de gestion de collectifs artistiques. **Toute la spécification est dans [PRD.md](PRD.md) — le lire avant toute action.**

- État : §17 étapes 1 à 9 implémentées. API Rust, front React, `layout/` (moteur de rendu partagé), `stills/`, `cli/`, bot Telegram, Docker Compose complet.
- Rien n'est encore commité ni déployé. Vérif manuelle du studio dans un navigateur à faire.
- Pile : Rust (Axum, sqlx) · React + Vite + TypeScript + Tailwind · PostgreSQL · Remotion · Typst · Docker Compose · mise
- Interface en français, code en anglais (glossaire en §21).
- `mise run check` avant tout push. Pas de CI. Push et déploiement manuels.
