-- Nova Horizon MVP schema (design v0.3)
-- Units: pos_* in wu; station ids in DB are content strings (st.*).

CREATE EXTENSION IF NOT EXISTS citext;

CREATE TABLE accounts (
    id            UUID PRIMARY KEY,
    email         CITEXT UNIQUE NOT NULL,
    password_hash TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    banned_until  TIMESTAMPTZ
);

CREATE TABLE characters (
    id                   UUID PRIMARY KEY,
    account_id           UUID NOT NULL REFERENCES accounts(id),
    name                 CITEXT UNIQUE NOT NULL,
    credits              BIGINT NOT NULL CHECK (credits >= 0),
    active_ship_id       UUID,
    faction_reps         JSONB NOT NULL DEFAULT '{}'::jsonb,
    trade_volume_day     BIGINT NOT NULL DEFAULT 0 CHECK (trade_volume_day >= 0),
    trade_volume_day_date DATE NOT NULL DEFAULT (CURRENT_DATE),
    created_at           TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ships (
    id                  UUID PRIMARY KEY,
    character_id        UUID NOT NULL REFERENCES characters(id),
    def_id              TEXT NOT NULL,
    name                TEXT,
    system_id           TEXT NOT NULL,
    pos_x               DOUBLE PRECISION NOT NULL,
    pos_y               DOUBLE PRECISION NOT NULL,
    rot                 REAL NOT NULL,
    shield              REAL NOT NULL,
    armor               REAL NOT NULL,
    energy              REAL NOT NULL,
    fuel                INT NOT NULL CHECK (fuel >= 0),
    loadout             JSONB NOT NULL DEFAULT '{}'::jsonb,
    docked_station      TEXT,
    last_docked_station TEXT,
    jump_state          TEXT,
    jump_dest           TEXT,
    jump_token          UUID,
    jump_updated_at     TIMESTAMPTZ,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT ships_jump_state_chk CHECK (
        jump_state IS NULL
        OR jump_state IN (
            'pending',
            'ingress_reserved',
            'persisted_egress',
            'token_issued',
            'limbo'
        )
    )
);

ALTER TABLE characters
    ADD CONSTRAINT characters_active_ship_fk
    FOREIGN KEY (active_ship_id) REFERENCES ships(id);

CREATE INDEX ships_character_id_idx ON ships (character_id);
CREATE INDEX ships_system_id_idx ON ships (system_id);
CREATE INDEX ships_jump_state_idx ON ships (jump_state)
    WHERE jump_state IS NOT NULL;

CREATE TABLE cargo_stacks (
    ship_id      UUID NOT NULL REFERENCES ships(id) ON DELETE CASCADE,
    commodity_id TEXT NOT NULL,
    quantity     INT NOT NULL CHECK (quantity > 0),
    PRIMARY KEY (ship_id, commodity_id)
);

CREATE TABLE station_markets (
    station_id   TEXT NOT NULL,
    commodity_id TEXT NOT NULL,
    stock        INT NOT NULL CHECK (stock >= 0),
    buy_price    INT NOT NULL CHECK (buy_price >= 0),
    sell_price   INT NOT NULL CHECK (sell_price >= 0),
    version      BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (station_id, commodity_id)
);

CREATE TABLE sessions (
    id           UUID PRIMARY KEY,
    account_id   UUID NOT NULL REFERENCES accounts(id),
    character_id UUID REFERENCES characters(id),
    refresh_hash TEXT NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    revoked_at   TIMESTAMPTZ,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX sessions_account_id_idx ON sessions (account_id);

-- At most one non-revoked play session per character.
CREATE UNIQUE INDEX sessions_one_live_character
    ON sessions (character_id)
    WHERE character_id IS NOT NULL AND revoked_at IS NULL;

CREATE TABLE economy_ledger (
    id            BIGSERIAL PRIMARY KEY,
    character_id  UUID NOT NULL,
    kind          TEXT NOT NULL,
    delta_credits BIGINT NOT NULL,
    payload       JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX economy_ledger_character_id_idx ON economy_ledger (character_id);

CREATE TABLE transfer_tokens (
    token        UUID PRIMARY KEY,
    character_id UUID NOT NULL,
    ship_id      UUID NOT NULL,
    dest_system  TEXT NOT NULL,
    payload      JSONB NOT NULL,
    expires_at   TIMESTAMPTZ NOT NULL,
    consumed_at  TIMESTAMPTZ
);

CREATE INDEX transfer_tokens_expires_idx ON transfer_tokens (expires_at)
    WHERE consumed_at IS NULL;
