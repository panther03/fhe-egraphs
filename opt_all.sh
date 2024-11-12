#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Please supply ruleset as 1st argument"
    exit 1
fi

# Ruleset
RULESET=$1

CASES=$(for c in $(ls bench/lobster); do echo "BENCH=$(basename $c .eqn)"; done)
# not the real timeout
export TIMEOUT=$((600000))
export RULESET=$RULESET
parallel -j 8 ./launch_opt.sh  ::: $CASES 