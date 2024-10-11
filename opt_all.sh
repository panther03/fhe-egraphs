#!/bin/bash

if [ $# -ne 1 ]
  then
    echo "Please supply ruleset as 1st argument"
    exit 1
fi

# Ruleset
RULESET=$1

CASES=$(for c in $(ls bench/missed); do echo "BENCH=$(basename $c .eqn)"; done)

# 30 mins
export TIMEOUT=$((30*60))
export RULESET=$RULESET
parallel ./launch_opt.sh  ::: $CASES 