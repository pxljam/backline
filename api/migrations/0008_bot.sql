-- Telegram bot state (§13). A member of several collectives works on one
-- collective at a time in the chat: `/collectif` switches it.
ALTER TABLE users
    ADD COLUMN telegram_collective_id UUID REFERENCES collectives(id) ON DELETE SET NULL;
