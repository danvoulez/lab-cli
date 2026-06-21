-- Migration: drop legacy parallel ledger
-- Date: 2026-06-21
-- Author: Santo Andre Laboratory
-- Risk: IRREVERSIBLE — deletes table and all rows in public.lab_log
--
-- Context:
--   The canonical ledger is public.logline_acts (logline.receipt.v0).
--   A legacy table public.lab_log was created in parallel and used by earlier
--   versions of the lab CLI. The CLI has been migrated to write exclusively to
--   public.logline_acts as canonical LogLine receipts.
--
--   This migration removes the deprecated table to enforce the single-ledger rule.
--
-- Preconditions before running:
--   1. All lab CLI binaries (lab-256, lab-512, lab-8gb) are updated to the
--      version that writes to public.logline_acts.
--   2. No other service, automation, or MCP tool reads from or writes to
--      public.lab_log.
--   3. You accept that historical rows in public.lab_log will NOT be migrated.
--      They will be permanently deleted.

BEGIN;

-- Verify the canonical ledger exists before dropping the legacy one.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'public' AND table_name = 'logline_acts'
    ) THEN
        RAISE EXCEPTION 'Canonical ledger public.logline_acts does not exist. Aborting.';
    END IF;
END $$;

-- Drop the legacy parallel ledger.
DROP TABLE IF EXISTS public.lab_log;

COMMIT;

-- Post-check: confirm only the canonical ledger remains among known Lab tables.
-- Expected: public.logline_acts exists, public.lab_log does not exist.
