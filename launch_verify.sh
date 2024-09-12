#!/bin/bash
# 2gb ram limit
ulimit -v 4000000000
timeout $TIMEOUT make verify $1