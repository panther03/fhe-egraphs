#!/bin/bash
# 2gb ram limit
#ulimit -v 200000
timeout $TIMEOUT make opt RULESET=$RULESET BENCHSET=$BENCHSET $1