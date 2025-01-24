#!/bin/bash

# Check if two arguments are provided
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <arg1> <arg2>"
    exit 1
fi

# Execute the command and check the output
output=$(abc -c "cec $1 $2" 2>&1)
exit_code=$?

if [ $exit_code -ne 0 ]; then
    echo "FAIL"
    exit 1;
fi

if echo "$output" | grep -q "Networks are equivalent"; then
    echo "PASS"
else
    echo "FAIL"
fi