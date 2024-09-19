#!/bin/bash
# 2gb ram limit
ulimit -v 40000000
timeout $TIMEOUT make verify $1