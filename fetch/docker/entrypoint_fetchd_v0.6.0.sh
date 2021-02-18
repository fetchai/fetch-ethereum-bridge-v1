#!/bin/bash
set -e 

sed -i 's/strict_tx_filtering = "true"/strict_tx_filtering = "false"/' ~/.fetchd/config/config.toml 
fetchd start | tee -a fetchd.logs
#fetchcli rest-server --laddr tcp://127.0.0.1:1317 | tee -a rest-server.logs
