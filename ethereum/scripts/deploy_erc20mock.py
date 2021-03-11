#!/usr/bin/env python3

from brownie import network, accounts, FetERC20Mock as Contract
from .deployment_common import (
    get_owner_account,
    get_deployment_manifest_path,
    load_network_manifest,
    save_network_manifest,
    NetworkManifest,
    )
from .deployment_manifest_schema import (
    NetworkManifest,
    )
from eth_account.account import (
    Account,
    )
import json


def deploy(network_manifest: NetworkManifest, owner: Account) -> Contract:
    contract_manif = network_manifest.FetERC20Mock

    constructor_params = contract_manif.constructor_parameters
    contract = Contract.deploy(
        constructor_params.name
        , constructor_params.symbol
        , constructor_params.initialSupply
        , constructor_params.decimals_
        , {'from': owner})
    # , {'from': owner, 'gas_price': '20 gwei'})

    contract_manif.contract_address = contract.address
    contract_manif.deployer_address = owner.address
    if hasattr(owner, 'public_key'):
        contract_manif.deployer_public_key = owner.public_key.to_hex()
    else:
        contract_manif.deployer_public_key = ""
        # contract_manif.pop("deployer_public_key", None)

    return contract


def transfer_funds_to_bridge_admin(contract: Contract, network_manifest: NetworkManifest, owner: Account) -> int:
    admin_address = network_manifest.Bridge.admin_address
    if admin_address:
        owner_funds = contract.balanceOf(owner)
        contract.transfer(admin_address, owner_funds, {'from': owner})
        print(f'FetERC20Mock{{{contract.address}}}.transfer({admin_address}, {owner_funds}, {{from: {owner.address}}})')
        return owner_funds

    return 0


def main():
    owner = get_owner_account()
    deployment_manifest_path = get_deployment_manifest_path()
    manifest, network_manif = load_network_manifest(deployment_manifest_path)
    print(f'network manifest: {network_manif}')

    contract = deploy(network_manif, owner)
    save_network_manifest(deployment_manifest_path, manifest, network_manif)
    #amount = transfer_funds_to_bridge_admin(contract=contract, network_manifest=network_manif, owner=owner)
    #print(f'Transferred {amount} Canonical FET from owner to admin.')


if __name__ == "__main__":
    main()
