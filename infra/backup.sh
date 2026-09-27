#!/bin/sh
# Daily backup, on the server (§15).
#
# Installed as a cron entry by `mise run vps:deploy`, and run again on demand by
# `mise run vps:backup` just before pulling. Both `pg_dump` and `mc mirror`
# stream, so a 4 GB box copes with a database it could not hold in memory.
#
#   cd ~/backline && ./infra/backup.sh
#
# What this does NOT do: copy anything off the machine. That is the pull, from
# the laptop — same choice as vps-infra, and the reason `vps:backup` exists.

set -eu

cd "$(dirname "$0")/.."
. ./.env

COMPOSE="docker compose -f docker-compose.yml -f compose.prod.yml --env-file .env"
DIR="$(pwd)/backups"
mkdir -p "$DIR/storage"

stamp=$(date +%F)
echo "--- $(date -Is) backup"

# The database. `-T` because cron has no tty.
$COMPOSE exec -T db pg_dump -U "${POSTGRES_USER:-backline}" "${POSTGRES_DB:-backline}" \
  | gzip > "$DIR/db-$stamp.sql.gz"
echo "db      $(du -h "$DIR/db-$stamp.sql.gz" | cut -f1)"

# The bucket: uploaded media are never regenerable (renders are).
$COMPOSE run --rm --no-deps -v "$DIR/storage:/mirror" --entrypoint /bin/sh storage-init \
  -c "mc alias set local http://storage:9000 ${MINIO_ROOT_USER:-backline} ${MINIO_ROOT_PASSWORD:-backline} >/dev/null \
      && mc mirror --overwrite --quiet local/${S3_BUCKET:-backline} /mirror" \
  || echo "storage mirror failed"
echo "storage $(du -sh "$DIR/storage" | cut -f1)"

# Retention: a fortnight of dailies, and the first of every month kept for good.
# A mistake noticed three weeks later still has something to go back to.
find "$DIR" -name 'db-*.sql.gz' -mtime +14 ! -name 'db-????-??-01.sql.gz' -delete
echo "kept    $(find "$DIR" -name 'db-*.sql.gz' | wc -l) dump(s)"
