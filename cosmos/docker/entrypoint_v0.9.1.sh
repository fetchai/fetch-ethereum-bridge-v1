#!/bin/bash
set -e 

echo $PATH
wasmd start | tee -a wasmd.logs &
wasmcli rest-server --laddr tcp://127.0.0.1:1317 | tee -a rest-server.logs
