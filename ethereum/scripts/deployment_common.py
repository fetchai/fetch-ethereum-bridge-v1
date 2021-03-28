import getpass
import os
import json
from brownie import (
    Bridge,
    FetERC20Mock,
    network,
    accounts,
    web3
    )
from eth_account.account import (
    Account,
    LocalAccount
    )
from typing import (
    Dict,
    Tuple
    )
from .deployment_manifest_schema import (
    NetworkManifest,
    ContractParamsBase,
    BridgeParams,
    Account as ManifestAccount,
    )


Address = str


def get_owner_account(
        priv_key_path_env_var: str = "DEPLOYMENT_PRIV_KEY_PATH",
        priv_key_pwd_env_var: str = "DEPLOYMENT_PRIV_KEY_PWD"):

    priv_key_path = os.environ.get(priv_key_path_env_var, None)

    if priv_key_path:
        _priv_key_path = os.path.abspath(os.path.expanduser(os.path.expandvars(priv_key_path)))
        # IF env var to key file is provided
        priv_key_pwd = os.environ.get(priv_key_pwd_env_var, None)
        if priv_key_pwd is None:
            priv_key_pwd = getpass.getpass("Password for private key: ")
        with open(_priv_key_path) as f:
            encr_pk_json = json.load(f)
        pk = Account.decrypt(encr_pk_json, priv_key_pwd)
        owner = accounts.add(pk)
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


def configure_bridge_contract(contract: Bridge, owner: Account, contract_manifest: BridgeParams):
    admin = contract_manifest.admin_wallet.address if contract_manifest.admin_wallet else None
    relayer = contract_manifest.relayer_wallet.address if contract_manifest.admin_wallet else None

    adminRole: bytes = 0
    relayerRole: bytes = web3.solidityKeccak(['string'], ["RELAYER_ROLE"])

    if relayer and web3.isAddress(relayer):
        contract.grantRole(relayerRole, relayer, {'from': owner})

    if admin and web3.isAddress(admin) and admin != owner.address:
        contract.grantRole(adminRole, admin, {'from': owner})
        contract.renounceRole(adminRole, owner, {'from': owner})

    transfer_eth_funds_to_admin_and_relayer(contract_manifest, owner)


def transfer_eth_funds_to_admin_and_relayer(bridge_manifest: BridgeParams, owner: Account) -> int:
    def fund_wallet(wallet: ManifestAccount, wallet_name):
        necessary_amount = 0

        if wallet and wallet.funding:
            wallet_orig_eth_balance = web3.eth.getBalance(wallet.address)
            if wallet_orig_eth_balance < wallet.funding:
                necessary_amount = wallet.funding - wallet_orig_eth_balance
                owner.transfer(wallet.address, necessary_amount)
                print(f'Owner {{{owner.address}}} transferred {necessary_amount} [1e-18 x ETH] to {wallet.address} "{wallet_name}" wallet.')
        return necessary_amount

    admin_wallet = bridge_manifest.admin_wallet
    relayer_wallet = bridge_manifest.relayer_wallet
    admin_added_funds = fund_wallet(admin_wallet, "admin_wallet")
    relayer_added_funds = fund_wallet(relayer_wallet, "relayer_wallet")

    return admin_added_funds, relayer_added_funds


def verify_etherscan_api_token_is_set(throw_exception: bool = True) -> bool:
    etherscan_toke_env_var = "ETHERSCAN_TOKEN"

    etherscan_token = os.environ.get(etherscan_toke_env_var, None)
    is_set = etherscan_token is not None

    if not is_set:
        msg = ((f'The {etherscan_toke_env_var} env var is NOT set. It is required for publishing & verification '
                f'of contract code on Etherscan.'))

        if throw_exception:
            raise RuntimeError(msg)
        else:
            print(msg)

    return is_set


def publish_contract_if_required(contract_container: network.contract.ContractContainer,
                                 contract: network.contract.ProjectContract,
                                 contract_manifest: ContractParamsBase,
                                 throw_exception: bool = True) -> bool:

    if not contract_manifest.publish_source:
        return True

    is_set = verify_etherscan_api_token_is_set(throw_exception)

    if is_set:
        try:
            contract_container.publish_source(contract)
            return True
        except Exception as ex:
            if throw_exception:
                raise

            print((f'[ERROR]: Publishing source of contract on Etherscan failed. This is NON-critical error, '
                   f'since contract source can be verified later manually. Exception: {ex}'))

            return False
    else:
        msg = ((f'[ERROR]: Contract source has NOT been published(as requested in manifest) due to missing'
                f' Etherscan API Token.\nThis is NON-critical error, since publishing can be done later.'))
        if throw_exception:
            raise RuntimeError(msg)

        print(msg)
        return False


def transfer_all_fet_tokens_to_bridge_admin(contract: FetERC20Mock, bridge_manifest: BridgeParams, owner: Account) -> int:
    admin_wallet = bridge_manifest.admin_wallet
    if admin_wallet:
        if not admin_wallet.address:
            raise ValueError(f'Mandatory address value not set for the "admin_wallet".')
        owner_funds = contract.balanceOf(owner)
        contract.transfer(admin_wallet.address, owner_funds, {'from': owner})
        print(f'FetERC20Mock{{{contract.address}}}.transfer({admin_wallet.address}, {owner_funds}, {{from: {owner.address}}})')
        return owner_funds

    return 0
