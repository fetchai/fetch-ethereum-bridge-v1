#!/bin/bash

set -e

wd=$(pwd)
root_dir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && cd .. && pwd )"

docker_image="fetch-contract-compiler"
contract_mount="/contract"

echo "[I] building docker image..."
docker build -t ${docker_image} -f ${root_dir}/docker/Dockerfile ${root_dir}

echo "[I] compiling contract"

run_tests=""
if [[ $* == *--test* ]];
then
    run_tests="${contract_mount} --test"
fi

docker run --init --rm  -v ${root_dir}/cosmwasm_contract:${contract_mount} -it fetch-contract-compiler ${run_tests}

mv -f ${root_dir}/cosmwasm_contract/bridge.wasm ${root_dir}
echo "[I] contract succefully compiled"
echo "${root_dir}/bridge.wasm"
