#!/usr/bin/python3

import pytest
import brownie
from brownie import MultisigCoordinator
from dataclasses import dataclass
from typing import Optional, Any, Union


def toBytes(value: Union[str, bytes, bytearray]) -> bytes:
    if isinstance(value, bytes):
        return value

    if isinstance(value, bytearray):
        return bytes(value)

    if isinstance(value, str):
        if value.startswith("0x") or value.startswith("0X") :
            return bytes.fromhex(value[2:])

    raise TypeError("Unable to convert input value to `bytes` type")


#from eth_keys.datatypes import Signature as Signature2
DEFAULT_ADMIN_ROLE = toBytes("0x0000000000000000000000000000000000000000000000000000000000000000")


@dataclass
class Setup:
    multisig_signatories = None
    threshold = None
    timeount = 10
    non_signatory = None


setup = Setup()


@pytest.fixture(scope="module", autouse=True)
def multisig_coord(MultisigCoordinator, accounts):
    committee_size = 4
    setup.threshold = 4
    assert committee_size >= setup.threshold > 0

    setup.owner = accounts[0].address
    setup.multisig_signatories = sorted([account.address for account in accounts[1:committee_size+1]])
    # The `executor` account intentionally selected outside of owner of contracts or signatories configured in multisig_coord
    setup.non_signatory = accounts[committee_size + 1].address

    contract = MultisigCoordinator.deploy(setup.multisig_signatories, setup.threshold, setup.timeount, {'from': setup.owner})

    yield contract


@pytest.fixture(autouse=True)
def isolate(fn_isolation):
    # perform a chain rewind after completing each test, to ensure proper isolation
    # https://eth-brownie.readthedocs.io/en/v1.10.3/tests-pytest-intro.html#isolation-fixtures
    pass


def test_initiation(multisig_coord, accounts):
    nextCoordinationNonce, threshold, timeout, committee = multisig_coord.getCoordinationSetup()

    assert nextCoordinationNonce == 1
    assert setup.threshold == threshold
    assert setup.timeount == timeout
    assert len(committee) == len(setup.multisig_signatories)

    committee = set(setup.multisig_signatories)
    for i in range(0, len(committee)):
        member = multisig_coord.committee(i)
        assert member in committee


    for member in committee:
        idx = multisig_coord.committeeMap(member)
        assert 0 < idx <= len(committee)
        assert member == multisig_coord.committee(idx - 1)


def test_propose(multisig_coord, accounts):
    expectd_action = 'xyz test action'
    expected_action_data = '{"a_key": "a value"}'
    expected_signature = '{"s": 3, "r": 2, "v": 1}'
    current_nonce = multisig_coord.coordinationNonce()
    expected_expired_since_block = brownie.web3.eth.blockNumber + setup.timeount + 1
    signee = setup.multisig_signatories[0]

    multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})

    coordinationNonce, \
    action, \
    actionData, \
    expiredSinceBlock, \
    signees, \
    signatures = multisig_coord.getSigningState()

    assert current_nonce + 1 == coordinationNonce
    assert expectd_action == action
    assert expected_action_data == actionData
    assert expected_expired_since_block == expiredSinceBlock
    assert signee in set(signees)
    assert expected_signature in set(signatures)


def test_propose_reverts_if_not_expired(multisig_coord, accounts):
    expectd_action = 'xyz test action'
    expected_action_data = '{"a_key": "a value"}'
    expected_signature = '{"s": 3, "r": 2, "v": 1}'
    current_nonce = multisig_coord.coordinationNonce()
    expected_expired_since_block = brownie.web3.eth.blockNumber + setup.timeount + 1
    signee = setup.multisig_signatories[0]

    multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})

    with brownie.reverts(revert_msg="expiration block NOT reached"):
        multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})


def test_propose_reverts_if_empty_action(multisig_coord, accounts):
    expectd_action = 'xyz test action'
    expected_action_data = '{"a_key": "a value"}'
    expected_signature = '{"s": 3, "r": 2, "v": 1}'
    current_nonce = multisig_coord.coordinationNonce()
    expected_expired_since_block = brownie.web3.eth.blockNumber + setup.timeount + 1
    signee = setup.multisig_signatories[0]

    with brownie.reverts(revert_msg="empty action"):
        multisig_coord.propose(current_nonce, '', expected_action_data, expected_signature, {'from': signee})

    # proving opposite
    multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})


def test_sign_reverts_if_already_voted(multisig_coord, accounts):
    expectd_action = 'xyz test action'
    expected_action_data = '{"a_key": "a value"}'
    expected_signature = '{"s": 3, "r": 2, "v": 1}'
    current_nonce = multisig_coord.coordinationNonce()
    expected_expired_since_block = brownie.web3.eth.blockNumber + setup.timeount + 1
    signee = setup.multisig_signatories[0]

    multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})

    with brownie.reverts(revert_msg="member already signed"):
        multisig_coord.sign(current_nonce + 1, expected_signature, {'from': signee})


def test_sign_from_whole_committee(multisig_coord, accounts):
    expectd_action = 'xyz test action'
    expected_action_data = '{"a_key": "a value"}'
    expected_signature = '{"s": 3, "r": 2, "v": 1}'
    current_nonce = multisig_coord.coordinationNonce()
    expected_expired_since_block = brownie.web3.eth.blockNumber + setup.timeount + 1
    signee = setup.multisig_signatories[0]


    signatures = [expected_signature]
    multisig_coord.propose(current_nonce, expectd_action, expected_action_data, expected_signature, {'from': signee})
    new_nonce = current_nonce + 1
    for s in setup.multisig_signatories[1:]:
        sig = f'signature:{s}'
        signatures.append(sig)
        multisig_coord.sign(new_nonce, sig, {'from': s})

    # proving opposite
    for s, sig in zip(setup.multisig_signatories, signatures):
        with brownie.reverts(revert_msg="member already signed"):
            multisig_coord.sign(new_nonce, sig, {'from': s})

