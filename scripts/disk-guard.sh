#!/bin/sh
# Abort big builds when the disk is nearly full (docs/web/07 "Disk-aware CI note").
free_kb=$(df -k . | awk 'NR==2 {print $4}')
if [ "${free_kb:-0}" -lt 3145728 ]; then
  echo "disk-guard: less than 3 GB free ($((free_kb / 1024)) MB). Free space (cargo clean) and retry." >&2
  exit 1
fi
