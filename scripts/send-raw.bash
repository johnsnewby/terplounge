#!/bin/bash

cat $1 | websocat -b -n "ws://localhost:3030/chat?lang=$2&rate=48000&resource=$3"
