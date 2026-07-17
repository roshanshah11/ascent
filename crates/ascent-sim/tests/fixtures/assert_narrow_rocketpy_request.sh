#!/bin/sh
request="$(cat)"
case "$request" in
  *'"manufacturer"'*|*'"provenance"'*|*'"expected_total_impulse_ns"'*|*'"diameter_mm"'*)
    echo "RocketPy request contained non-bridge motor metadata" >&2
    exit 2
    ;;
esac
script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
cat "$script_dir/../../../../bridges/rocketpy/reference_output.json"
