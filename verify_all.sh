#!/bin/bash

if [ $# -ne 2 ]
  then
    echo "Usage: $0 <OPTDIR> <BENCHSET>"
    exit 1
fi

# Folder to evaluate
OPTDIR=$1
# BENCHSET to check equivalence against
BENCHSET=$2


CASES=$(for c in $(ls "${OPTDIR}"/); do echo "BENCH=$(basename $c .eqn)"; done)

parallel make stats verify OPTDIR=$OPTDIR BENCHSET=$BENCHSET ::: $CASES