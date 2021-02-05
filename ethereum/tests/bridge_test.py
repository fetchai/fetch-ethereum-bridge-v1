#!/usr/bin/python3
import pprint
import pytest
import brownie
from brownie import FetERC20Mock, BridgeMock
from dataclasses import dataclass
from typing import Optional, Any

#from eth_keys.datatypes import Signature as Signature2

@dataclass
class Setup:
    tokenDecimals = 18
    #tokenCanonMultip = 10**tokenDecimals
    #totalSupply = 1000000 * tokenCanonMultip
    #userFunds = 1000 * tokenCanonMultip
    totalSupply = 10**10
    userFunds = 1000
    amount = 10
    cap = 1000
    upperSwapLimit = 100
    lowerSwapLimit = 10
    swapFeee = 5
    pausedSincelBlock = 0xffffffffffffffff
    deleteProtectionPeriod = 13
    deploymentBlockNumber = None
    owner = None
    relayer = None
    users = None

    dest_swap_address = "some weird encoded and loooooonooooooooger than normal address"
    dest_swap_address_hash = brownie.web3.solidityKeccak(["string"], [dest_swap_address])
    src_tx_hash = brownie.web3.solidityKeccak(["string"], ["some tx has"])

    adminRole = 0
    relayerRole = brownie.web3.solidityKeccak(['string'], ["RELAYER_ROLE"])
    delegateRole = brownie.web3.solidityKeccak(['string'], ["DELEGATE_ROLE"])

setup = Setup()


@pytest.fixture(scope="module", autouse=True)
def token(FetERC20Mock, accounts):
    setup.owner = accounts[0]
    setup.relayer = accounts[1]
    setup.users = accounts[2:]

    contract = FetERC20Mock.deploy("Fetch", "FET", setup.totalSupply, setup.tokenDecimals, {'from': setup.owner})

    for user in setup.users:
        contract.transfer(user, setup.userFunds)

    yield contract


@pytest.fixture(scope="module", autouse=True)
def bridge(BridgeMock, token, accounts):
    contract = BridgeMock.deploy(
        token.address,
        setup.cap,
        setup.upperSwapLimit,
        setup.lowerSwapLimit,
        setup.swapFeee,
        setup.pausedSincelBlock,
        setup.deleteProtectionPeriod,
        {'from': setup.owner})

    setup.deploymentBlockNumber = contract.blockNumber()
    contract.grantRole(setup.relayerRole, setup.relayer.address, {'from': setup.owner})

    yield contract


@pytest.fixture(autouse=True)
def isolate(fn_isolation):
    # perform a chain rewind after completing each test, to ensure proper isolation
    # https://eth-brownie.readthedocs.io/en/v1.10.3/tests-pytest-intro.html#isolation-fixtures
    pass


def swap(bridge, token, user, amount: int = setup.amount, dest_addr: str = setup.dest_swap_address):
    origSwapId = bridge.nextSwapId()
    orig_bridge_supply = bridge.supply()
    orig_bridge_balance = token.balanceOf(bridge)
    orig_user_balance = token.balanceOf(user)
    #assert origBal >= amount

    token.approve(bridge, amount, {'from': user})
    assert token.allowance(user, bridge) == amount

    tx = bridge.swap(amount, dest_addr, {'from': user})

    assert bridge.nextSwapId() == origSwapId + 1
    assert bridge.supply() == orig_bridge_supply + amount
    assert token.balanceOf(bridge) == orig_bridge_balance + amount
    assert token.balanceOf(user) == orig_user_balance - amount

    event = tx.events['Swap']
    assert event['id'] == origSwapId
    assert brownie.convert.to_bytes(event['indexedTo'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [dest_addr])
    assert event['to'] == dest_addr
    assert event['amount'] == amount

    return tx


def refund(bridge,
           token,
           id: int,
           to_user,
           amount: int = setup.amount,
           relay_eon = None,
           caller=None):

    swapFee = bridge.swapFee()
    caller = caller or setup.relayer
    relay_eon = relay_eon or bridge.relayEon()
    orig_refunds_fees_accrued = bridge.refundsFeesAccrued()
    orig_bridge_supply = bridge.supply()
    orig_bridge_balance = token.balanceOf(bridge)
    orig_user_balance = token.balanceOf(to_user)

    effective_fee = swapFee if amount > swapFee else amount
    refunded_amount = amount - effective_fee

    #assert bridge.refunds(id) == 0
    tx = bridge.refund(id, to_user, amount, relay_eon, {'from': caller})

    assert bridge.supply() == orig_bridge_supply - amount
    assert bridge.refundsFeesAccrued() == orig_refunds_fees_accrued + effective_fee
    assert bridge.refunds(id) == amount

    assert token.balanceOf(bridge) == orig_bridge_balance - refunded_amount
    assert token.balanceOf(to_user) == orig_user_balance + refunded_amount

    event = tx.events['SwapRefund']
    assert event['id'] == id
    assert event['to'] == to_user
    assert event['refundedAmount'] == refunded_amount
    assert event['fee'] == swapFee

    return tx


def revereseSwap(bridge,
                 token,
                 rid: int,
                 to_user,
                 amount: int = setup.amount,
                 origin_from: str = setup.dest_swap_address,
                 origin_tx_hash = setup.src_tx_hash,
                 relay_eon=None,
                 caller=None):
    swapFee = bridge.swapFee()
    caller = caller or setup.relayer
    orig_refunds_fees_accrued = bridge.refundsFeesAccrued()
    relay_eon = relay_eon or bridge.relayEon()
    orig_bridge_supply = bridge.supply()
    orig_bridge_balance = token.balanceOf(bridge)
    orig_user_balance = token.balanceOf(to_user)

    effective_amount = amount - swapFee if amount > swapFee else 0

    tx = bridge.reverseSwap(rid, to_user, origin_from, origin_tx_hash, amount, relay_eon, {'from': caller})

    assert bridge.supply() == orig_bridge_supply - effective_amount
    assert bridge.refundsFeesAccrued() == orig_refunds_fees_accrued
    assert token.balanceOf(bridge) == orig_bridge_balance - effective_amount
    assert token.balanceOf(to_user) == orig_user_balance + effective_amount

    event = tx.events['ReverseSwap']
    assert event['rid'] == rid
    assert event['to'] == to_user
    assert brownie.convert.to_bytes(event['from'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [origin_from])
    assert brownie.convert.to_bytes(event['originTxHash'], 'bytes32') == origin_tx_hash
    assert event['effectiveAmount'] == effective_amount
    assert event['fee'] == swapFee

    return tx


def test_initial_state(bridge, token):
    assert bridge.relayEon() == ((1<<64)-1)
    assert bridge.nextSwapId() == 0
    assert bridge.refundsFeesAccrued() == 0
    assert bridge.token() == token
    assert bridge.earliestDelete() == setup.deploymentBlockNumber + setup.deleteProtectionPeriod
    assert bridge.refundsFeesAccrued() == 0
    assert bridge.refundsFeesAccrued() == 0


def test_newRelayEon_basic(bridge):
    tx = bridge.newRelayEon({'from': setup.relayer})
    assert bridge.relayEon() == 0
    evName = 'NewRelayEon'
    assert evName in tx.events
    assert tx.events[evName]['eon'] == 0


def test_swap_basic(bridge, token):
    swap(bridge, token, user=setup.users[0])


def test_reverseSwap_basic(bridge, token):
    user = setup.users[0]
    amount = setup.amount
    swap(bridge, token, user=user, amount=amount)
    revereseSwap(bridge, token, rid=0, to_user=user, amount=amount)


def test_refund_bacis(bridge, token):
    user = setup.users[0]
    amount = setup.amount
    swap_tx = swap(bridge, token, user=user, amount=amount)
    refund(bridge, token, id=swap_tx.events['Swap']['id'], to_user=user, amount=amount)

def test_refund_amount_smaller_than_fee(bridge, token):
    user = setup.users[0]
    amount = setup.amount
    swap_tx = swap(bridge, token, user=user, amount=amount)
    refund(bridge, token, id=swap_tx.events['Swap']['id'], to_user=user, amount=amount)
