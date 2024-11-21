#!/bin/bash

if [ $# -ne 2 ]
  then
    echo "Usage: $0 <RULESET> <BENCHSET>"
    exit 1
fi

# Ruleset
RULESET=$1
BENCHSET=$2

CASES=$(for c in $(ls bench/$BENCHSET); do echo "BENCH=$(basename $c .eqn)"; done)
# not the real timeout
export TIMEOUT=$((600000))
export RULESET=$RULESET
export BENCHSET=$BENCHSET
parallel -j 8 ./launch_opt.sh  ::: $CASES 