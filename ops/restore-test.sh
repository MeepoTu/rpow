#!/usr/bin/env bash
# Restore the latest backup into a scratch DB and assert row counts.
# This is the proof-of-life for the backup system.
set -euo pipefail

# shellcheck disable=SC1091
source /etc/rpow/restic.env
export B2_ACCOUNT_ID B2_ACCOUNT_KEY RESTIC_REPOSITORY RESTIC_PASSWORD

SCRATCH=rpow_restore_test
sudo -u postgres dropdb --if-exists "$SCRATCH"
sudo -u postgres createdb -O rpow_app "$SCRATCH"

LATEST=$(restic snapshots --tag rpow --json | jq -r 'sort_by(.time) | .[-1].id')
DUMP_PATH=$(restic snapshots --tag rpow --json | jq -r 'sort_by(.time) | .[-1].paths[0]')
echo "Restoring snapshot $LATEST ($DUMP_PATH)..."

restic dump "$LATEST" "$DUMP_PATH" \
    | sudo -u postgres pg_restore --no-owner --no-privileges -d "$SCRATCH"

echo "Row counts on restored scratch DB:"
sudo -u postgres psql -d "$SCRATCH" -c "
  SELECT 'users' AS tbl, count(*) FROM users
  UNION ALL SELECT 'tokens',         count(*) FROM tokens
  UNION ALL SELECT 'transfers',      count(*) FROM transfers
  UNION ALL SELECT 'magic_links',    count(*) FROM magic_links
  UNION ALL SELECT 'challenges',     count(*) FROM challenges
  UNION ALL SELECT 'pending_transfers', count(*) FROM pending_transfers
  ORDER BY tbl;
"

sudo -u postgres dropdb "$SCRATCH"
echo "Restore drill OK."
