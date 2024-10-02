#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Please supply ruleset as 1st argument"
    exit 1
fi

# Ruleset
RULESET=$1

CASES=$(for c in $(ls bench/); do echo "BENCH=$(basename $c .eqn) RULESET=$1"; done)

# 30 mins
export TIMEOUT=$((30*60))
parallel ./launch_opt.sh ::: $CASES 