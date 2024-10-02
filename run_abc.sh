#!/bin/bash
if abc -c "cec bench/$1.eqn out/$1-opt.eqn" > /dev/null 2>&1; then
    echo "PASS"
else
    echo "FAIL"
fi