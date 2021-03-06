import os
import json
from dataclasses import dataclass
from dataclasses_json import dataclass_json
from brownie import Bridge, network, accounts, web3
from eth_account.account import (
    Account,
    )
from typing import Dict, Tuple
from .deployment_manifest_schema import NetworkManifest


def get_owner_account(priv_key_path_env_var: str = "PRIV_KEY_PATH"):
    if priv_key_path_env_var in os.environ:
        # IF env var to key file is provided
        private_key_file = os.environ.get(priv_key_path_env_var)
        owner = accounts.load(private_key_file)
    else:
        # If not use default accounts
        owner = accounts[0]

    print(f'owner: {owner}')
    return owner


def get_deployment_manifest_path(deployment_manifest_path_env_var: str = "ETH_CONTRACT_DEPLOYMENT_MANIFEST_PATH"):
    if deployment_manifest_path_env_var in os.environ:
        # IF env var to key file is provided
        deployment_manifest_path = os.path.abspath(
            os.path.expanduser(os.path.expandvars(os.environ.get(deployment_manifest_path_env_var))))
    else:
        # If not provided, use default:
        base_dir = os.path.dirname(os.path.abspath(__file__))
        deployment_manifest_filename = "deployment_manifest.json"
        deployment_manifest_path = os.path.join(base_dir, deployment_manifest_filename)

    print(f'deployment manifest file path: {deployment_manifest_path}')
    return deployment_manifest_path


Manifest = Dict


def load_deployment_manifest(deployment_manifest_path: str) -> Manifest:
    with open(deployment_manifest_path, mode="r") as f:
        manifest = json.load(f)
        return manifest


def load_network_manifest(deployment_manifest_path: str, _network:str = None) -> Tuple[Manifest, NetworkManifest]:
    manifest = load_deployment_manifest(deployment_manifest_path)
    _network = _network or network.show_active()
    network_manif_dict = manifest[_network]
    network_manif = NetworkManifest.from_dict(network_manif_dict)
    return manifest, network_manif


def save_network_manifest(
        deployment_manifest_path: str,
        manifest: Manifest,
        network_manifest:NetworkManifest,
        _network:str = None) -> Tuple[Manifest, NetworkManifest]:
    print(f'network nanifest: ', network_manifest)

    _network = _network or network.show_active()
    manifest[_network] = network_manifest.to_dict()
    with open(deployment_manifest_path, mode='w') as f:
        json.dump(manifest, f, indent=4)


def configure_bridge_contract(contract: Bridge, owner: Account, contract_manifest: dict):
    admin = contract_manifest.admin_address
    relayer = contract_manifest.relayer_address

    adminRole: bytes = 0
    relayerRole: bytes = web3.solidityKeccak(['string'], ["RELAYER_ROLE"])

    if relayer:
        contract.grantRole(relayerRole, relayer, {'from': owner})

    if admin and web3.isAddress(admin) and admin != owner.address:
        contract.grantRole(adminRole, admin, {'from': owner})
        contract.renounceRole(adminRole, owner, {'from': owner})
