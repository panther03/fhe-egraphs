#!/bin/bash

if [ $# -ne 3 ]
  then
    echo "Usage: $0 <RULESET> <BENCHSET> <JOBS>"
    exit 1
fi

# Ruleset
RULESET=$1
BENCHSET=$2
JOBS=$3

CASES=$(for c in $(ls bench/$BENCHSET); do echo "BENCH=$(basename $c .sexpr)"; done)
echo $CASES
#CASES=$(for c in $(ls bench/$BENCHSET); do echo "BENCH=$(basename $c .eqn)"; done)
# not the real timeout
export TIMEOUT=$((600000))
export RULESET=$RULESET
export BENCHSET=$BENCHSET
parallel -j $JOBS  ./scripts/launch_opt.sh  ::: $CASES 