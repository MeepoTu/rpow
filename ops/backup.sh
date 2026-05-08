#!/usr/bin/env bash
# nightly rpow Postgres → B2 backup. Pipes pg_dump straight into restic.
set -euo pipefail

# shellcheck disable=SC1091
source /etc/rpow/restic.env
export B2_ACCOUNT_ID B2_ACCOUNT_KEY RESTIC_REPOSITORY RESTIC_PASSWORD

LABEL="rpow-$(date -u +%FT%H%MZ).dump"

sudo -u postgres pg_dump -Fc rpow \
    | restic backup --stdin --stdin-filename "$LABEL" \
        --tag rpow --tag postgres

# retention: 7 daily, 4 weekly, 6 monthly
restic forget --tag rpow --keep-daily 7 --keep-weekly 4 --keep-monthly 6 --prune

# integrity check: read 5% of data on each run
restic check --read-data-subset=5%
