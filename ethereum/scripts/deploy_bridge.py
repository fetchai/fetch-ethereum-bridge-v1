#!/usr/bin/env python3

from brownie import network, accounts, Bridge as Contract
from .deploy_erc20mock import (
    deploy as deploy_erc20mock,
    )
from .deployment_common import (
    get_owner_account,
    get_deployment_manifest_path,
    configure_bridge_contract,
    load_network_manifest,
    save_network_manifest,
    transfer_all_fet_tokens_to_bridge_admin,
    transfer_eth_funds_to_admin_and_relayer,
    )
from .deployment_manifest_schema import (
    NetworkManifest,
    )
from eth_account.account import (
    Account,
)
import json
from typing import Dict


def deploy(network_manifest: NetworkManifest, owner: Account) -> Contract:
    bridge_manif = network_manifest.Bridge

    constructor_params = bridge_manif.constructor_parameters
    if constructor_params.ERC20Address == "":
        erc20mock_manif = network_manifest.FetERC20Mock
        erc20address = erc20mock_manif.contract_address
        if not erc20address:
            fetERC20Contract = deploy_erc20mock(network_manifest, owner)
            erc20address = erc20mock_manif.contract_address
            if erc20address == "":
                print("Deployment of ERC20 Mock contract failed.")
                exit

            transfer_all_fet_tokens_to_bridge_admin(fetERC20Contract, bridge_manifest=network_manifest.Bridge, owner=owner)

        constructor_params.ERC20Address = erc20address

    contract = Contract.deploy(
          constructor_params.ERC20Address
        , constructor_params.cap
        , constructor_params.upperSwapLimit
        , constructor_params.lowerSwapLimit
        , constructor_params.swapFee
        , constructor_params.pausedSinceBlock
        , constructor_params.deleteProtectionPeriod
        , {'from': owner})
        #, {'from': owner, 'gas_price': '20 gwei'})

    bridge_manif.contract_address = contract.address
    bridge_manif.deployer_address = owner.address
    if hasattr(owner, 'public_key'):
        bridge_manif.deployer_public_key = owner.public_key.to_hex()
    else:
        bridge_manif.deployer_public_key = ""
        #contract_manif.pop("deployer_public_key", None)

    return contract


def main():
    owner = get_owner_account()
    deployment_manifest_path = get_deployment_manifest_path()
    manifest, network_manif = load_network_manifest(deployment_manifest_path)
    print(f'network manifest: {network_manif}')

    contract = deploy(network_manif, owner)

    configure_bridge_contract(contract=contract, owner=owner, contract_manifest=network_manif.Bridge)
    save_network_manifest(deployment_manifest_path, manifest, network_manif)


if __name__ == "__main__":
    main()
