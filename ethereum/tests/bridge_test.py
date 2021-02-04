#!/usr/bin/python3
import pprint
import pytest
import brownie
from brownie import FetERC20Mock, Bridge
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
    deleteProtectionPeriod = 10
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
def bridge(Bridge, token, accounts):
    contract = Bridge.deploy(
        token.address,
        setup.cap,
        setup.upperSwapLimit,
        setup.lowerSwapLimit,
        setup.swapFeee,
        setup.pausedSincelBlock,
        setup.deleteProtectionPeriod,
        {'from': setup.owner})

    #print(f'contract relayer role: {contract.RELAYER_ROLE()}, calculated: {setup.relayerRole}')

    contract.grantRole(setup.relayerRole, setup.relayer.address, {'from': setup.owner})

    #assert contract.getRoleMemberCount(setup.relayerRole) == 1
    #assert contract.getRoleMember(setup.relayerRole, 0) == setup.relayer
    #assert contract.hasRole(setup.relayerRole, setup.relayer)

    yield contract


@pytest.fixture(autouse=True)
def isolate(fn_isolation):
    # perform a chain rewind after completing each test, to ensure proper isolation
    # https://eth-brownie.readthedocs.io/en/v1.10.3/tests-pytest-intro.html#isolation-fixtures
    pass


def swap(bridge, token, user, amount: int = setup.amount, dest_addr: str = "a dest addr"):
    origSwapId = bridge.nextSwapId()
    origSupply = bridge.supply()
    origBridgeBal = token.balanceOf(bridge)
    origUserBal = token.balanceOf(user)
    #assert origBal >= amount

    token.approve(bridge, amount, {'from': user})
    assert token.allowance(user, bridge) == amount

    tx = bridge.swap(amount, dest_addr, {'from': user})

    assert bridge.nextSwapId() == origSwapId + 1
    assert bridge.supply() == origSupply + amount
    assert token.balanceOf(bridge) == origBridgeBal + amount
    assert token.balanceOf(user) == origUserBal - amount

    event = tx.events['Swap']
    assert event['id'] == origSwapId
    assert brownie.convert.to_bytes(event['indexedTo'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [dest_addr])
    assert event['to'] == dest_addr
    assert event['amount'] == amount

    return tx


def revereseSwap(bridge,
                 token,
                 rid: int,
                 to_user,
                 amount: int = setup.amount,
                 origin_from: str = setup.dest_swap_address,
                 origin_tx_hash = setup.src_tx_hash,
                 caller=None,
                 relay_eon = None):
    swapFee = bridge.swapFee()
    caller = caller or setup.relayer
    relay_eon = relay_eon or bridge.relayEon()
    origSupply = bridge.supply()
    origBridgeBal = token.balanceOf(bridge)
    origUserBal = token.balanceOf(to_user)

    effectiveAmount = amount - swapFee if amount > swapFee else 0

    tx = bridge.reverseSwap(rid, to_user, origin_from, origin_tx_hash, amount, relay_eon, {'from': caller})

    assert bridge.supply() == origSupply - effectiveAmount
    assert token.balanceOf(bridge) == origBridgeBal - effectiveAmount
    assert token.balanceOf(to_user) == origUserBal + effectiveAmount

    event = tx.events['ReverseSwap']
    assert event['rid'] == rid
    assert event['to'] == to_user
    assert brownie.convert.to_bytes(event['from'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [origin_from])
    assert brownie.convert.to_bytes(event['originTxHash'], 'bytes32') == origin_tx_hash
    assert event['effectiveAmount'] == effectiveAmount
    assert event['fee'] == swapFee

    return tx


def test_initialState(bridge):
    assert bridge.relayEon() == ((1<<64)-1)
    assert bridge.nextSwapId() == 0


def test_firstNewRelayEon(bridge):
    tx = bridge.newRelayEon({'from': setup.relayer})
    assert bridge.relayEon() == 0
    evName = 'NewRelayEon'
    assert evName in tx.events
    assert tx.events[evName]['eon'] == 0


def test_basicSwap(bridge, token):
    swap(bridge, token, user=setup.users[0])


def test_basicReverseSwap(bridge, token):
    user = setup.users[0]
    amount = setup.amount
    swap(bridge, token, user=user, amount=amount)
    revereseSwap(bridge, token, rid=0, to_user=user, amount=amount)
