#!/bin/sh
set -e

# The /app/data volume is mounted from the host.
# Depending on the host context, it might be owned by root.
# We fix the ownership here while we still have root privileges.
chown -R dashboard:dashboard /app/data

# Execute the CMD instruction dropping privileges to the 'dashboard' user
exec su-exec dashboard "$@"
