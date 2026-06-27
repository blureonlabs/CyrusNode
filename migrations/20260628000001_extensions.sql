-- Migration 001: Enable required Postgres extensions.
-- pgcrypto provides gen_random_uuid() used as the default for all primary keys.
-- Postgres 14+ ships pgcrypto, but the extension must still be explicitly created.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
