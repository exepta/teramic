#!/usr/bin/bash
set -euo pipefail
swap_device=/dev/zram0
read -r swap_size_kib swap_used_kib swap_priority < <(awk '$1 == "/dev/zram0" { print $3, $4, $5 }' /proc/swaps)
read -r swap_original swap_compressed swap_physical swap_rest < /sys/block/zram0/mm_stat
available_kib=$(awk '$1 == "MemAvailable:" { print $2 }' /proc/meminfo)
# zram storage is released while pages return to RAM. Keep 2 GiB in reserve.
required_bytes=$((swap_used_kib * 1024 - swap_physical + 2147483648))
if (( available_kib * 1024 < required_bytes )); then
  printf 'Swap retained: available RAM does not cover the uncompressed pages and a 2 GiB reserve.\n'
  exit 2
fi
if [[ "${1-}" == --check ]]; then
  printf 'RAM check passed; targeted swap reset is ready.\n'
  exit 0
fi
if (( EUID != 0 )); then
  printf 'Administrator authentication is required.\n' >&2
  exit 3
fi
restore_swap() {
  if ! awk '$1 == "/dev/zram0" { found = 1 } END { exit !found }' /proc/swaps; then
    /usr/bin/swapon --priority "$swap_priority" "$swap_device"
  fi
}
trap restore_swap EXIT
/usr/bin/swapoff "$swap_device"
restore_swap
trap - EXIT
printf 'Targeted zram swap reset completed; original priority restored.\n'
/usr/bin/swapon --show
/usr/bin/free -h
