-- Etat du bot Telegram (§13). Un membre de plusieurs collectifs travaille sur
-- un collectif a la fois dans la conversation : `/collectif` en change.
ALTER TABLE users
    ADD COLUMN telegram_collective_id UUID REFERENCES collectives(id) ON DELETE SET NULL;
