#!/bin/bash
CASES=$(for c in $(ls lobster_bench/); do echo "BENCH=$(basename $c .eqn)"; done)
# 30 mins
export TIMEOUT=$((30*60))
parallel ./launch_verify.sh ::: $CASES 