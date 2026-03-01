#!/bin/sh
set -e

PUID=${PUID:-1000}
PGID=${PGID:-1000}

# Update the dashboard user's UID and GID to match the host user
groupmod -o -g "$PGID" dashboard
usermod -o -u "$PUID" dashboard

# The /app/data volume is mounted from the host.
# Depending on the host context, it might be owned by root or an old UID.
# We fix the ownership here while we still have root privileges.
chown -R dashboard:dashboard /app/data

# Execute the CMD instruction dropping privileges to the 'dashboard' user
exec su-exec dashboard "$@"
