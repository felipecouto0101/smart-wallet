-- Users
CREATE TABLE IF NOT EXISTS users (
    id            UUID PRIMARY KEY,
    name          VARCHAR(100) NOT NULL,
    email         VARCHAR(255) NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Wallets (1:1 with user)
CREATE TABLE IF NOT EXISTS wallets (
    user_id       UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    balance_cents BIGINT NOT NULL DEFAULT 0 CHECK (balance_cents >= 0)
);

-- Asset positions per user
CREATE TABLE IF NOT EXISTS positions (
    user_id  UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    symbol   VARCHAR(10) NOT NULL,
    quantity BIGINT NOT NULL DEFAULT 0 CHECK (quantity >= 0),
    PRIMARY KEY (user_id, symbol)
);

-- Executed order history
CREATE TABLE IF NOT EXISTS orders (
    id            UUID PRIMARY KEY,
    user_id       UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    order_type    VARCHAR(10) NOT NULL CHECK (order_type IN ('Buy', 'Sell')),
    symbol        VARCHAR(10) NOT NULL,
    quantity      BIGINT NOT NULL,
    price_cents   BIGINT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
