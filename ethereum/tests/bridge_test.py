#!/usr/bin/env python3

import pprint
import pytest
import brownie
from brownie import FetERC20Mock, Bridge
from dataclasses import dataclass, InitVar
from enum import Enum, auto
from typing import Type


CanonicalFET = Type[int]


class AutoNameEnum(Enum):
    def _generate_next_value_(name, start, count, last_values):
        return name

    def __str__(self):
        return self.value.split('.')[-1]


class EventType(AutoNameEnum):
    Swap = auto()
    SwapRefund = auto()
    ReverseSwap = auto()
    Pause = auto()
    LimitsUpdate = auto()
    CapUpdate = auto()
    NewRelayEon = auto()
    Withdraw = auto()
    Deposit = auto()
    RefundsFeesWithdrawal = auto()
    ExcessFundsWithdrawal = auto()
    DeleteContract = auto()


@dataclass
class TokenSetup:
    tokenDecimals: int = 18
    multiplier: int = 10**tokenDecimals
    totalSupply: int = None
    userFunds: int = None

    def toCanonical(self, amount_fet: int ) -> CanonicalFET:
        return amount_fet * self.multiplier

    def __post_init__(self):
        self.totalSupply = self.toCanonical(1000000)
        self.userFunds = self.toCanonical(1000)


@dataclass
class UsersSetup:
    owner = None
    relayer = None
    users = None
    adminRole: bytes = 0
    relayerRole: bytes = brownie.web3.solidityKeccak(['string'], ["RELAYER_ROLE"])
    delegateRole: bytes = brownie.web3.solidityKeccak(['string'], ["DELEGATE_ROLE"])


@dataclass
class BridgeSetup:
    token: InitVar[TokenSetup]
    cap: int = None
    upperSwapLimit: int = None
    lowerSwapLimit: int = None
    swapFee: int = None
    pausedSinceBlock: int = 0xffffffffffffffff
    pausedSinceBlockEffective: int = None
    deleteProtectionPeriod: int = 13
    deploymentBlockNumber: int = None

    def __post_init__(self, token):
        self.cap = token.toCanonical(1000)
        self.upperSwapLimit = token.toCanonical(100)
        self.lowerSwapLimit = token.toCanonical(10)
        self.swapFee = token.toCanonical(5)


@dataclass
class ValuesSetup:
    token: InitVar[TokenSetup]
    amount: int = None
    dest_swap_address = "some weird encoded and loooooonooooooooger than normal address"
    dest_swap_address_hash = brownie.web3.solidityKeccak(["string"], [dest_swap_address])
    src_tx_hash = brownie.web3.solidityKeccak(["string"], ["some tx has"])

    def __post_init__(self, token):
        self.amount = token.toCanonical(10)


@dataclass
class Setup__:
    users: UsersSetup = UsersSetup()
    token: TokenSetup = TokenSetup()
    bridge: BridgeSetup = None
    vals: ValuesSetup = None

    def __post_init__(self):
        self.bridge = BridgeSetup(self.token)
        self.vals = ValuesSetup(self.token)


@dataclass()
class BridgeTest:
    users: UsersSetup = UsersSetup()
    token: TokenSetup = TokenSetup()
    bridge: BridgeSetup = BridgeSetup(token)
    vals: ValuesSetup = ValuesSetup(token)
    t: FetERC20Mock = None
    b: Bridge = None


    def swap(self, user, amount: int = None, dest_addr: str = None):
        amount = self.vals.amount if amount is None else amount
        dest_addr = self.vals.dest_swap_address if dest_addr is None else dest_addr

        origSwapId = self.b.nextSwapId()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(user)
        #assert origBal >= amount

        self.t.approve(self.b, amount, {'from': user})
        assert self.t.allowance(user, self.b) == amount

        tx = self.b.swap(amount, dest_addr, {'from': user})

        assert self.b.nextSwapId() == origSwapId + 1
        assert self.b.supply() == orig_bridge_supply + amount
        assert self.t.balanceOf(self.b) == orig_bridge_balance + amount
        assert self.t.balanceOf(user) == orig_user_balance - amount

        event = tx.events['Swap']
        assert event['id'] == origSwapId
        assert brownie.convert.to_bytes(event['indexedTo'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [dest_addr])
        assert event['to'] == dest_addr
        assert event['amount'] == amount

        return tx


    def refund(self,
               id: int,
               to_user,
               amount: int = None,
               waive_fee = False,
               relay_eon = None,
               caller = None):

        amount = self.vals.amount if amount is None else amount
        relay_eon = self.b.relayEon() if relay_eon is None else relay_eon
        caller = caller or self.users.relayer
        swapFee = 0 if waive_fee else self.b.swapFee()

        orig_refunds_fees_accrued = self.b.refundsFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(to_user)

        effective_fee = swapFee if amount > swapFee else amount
        refunded_amount = amount - effective_fee

        #assert self.b.refunds(id) == 0
        if waive_fee:
            tx = self.b.refundInFull(id, to_user, amount, relay_eon, {'from': caller})
        else:
            tx = self.b.refund(id, to_user, amount, relay_eon, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.refundsFeesAccrued() == orig_refunds_fees_accrued + effective_fee
        assert self.b.refunds(id) == amount

        assert self.t.balanceOf(self.b) == orig_bridge_balance - refunded_amount
        assert self.t.balanceOf(to_user) == orig_user_balance + refunded_amount

        event = tx.events['SwapRefund']
        assert event['id'] == id
        assert event['to'] == to_user
        assert event['refundedAmount'] == refunded_amount
        assert event['fee'] == effective_fee

        return tx


    def revereseSwap(self,
                     rid: int,
                     to_user,
                     amount: int = None,
                     origin_from: str = None,
                     origin_tx_hash = None,
                     relay_eon = None,
                     caller = None):

        amount = self.vals.amount if amount is None else amount
        origin_from = self.vals.dest_swap_address if origin_from is None else origin_from
        origin_tx_hash = self.vals.src_tx_hash if origin_tx_hash is None else origin_tx_hash
        relay_eon = self.b.relayEon() if relay_eon is None else relay_eon
        caller = caller or self.users.relayer
        swapFee = self.b.swapFee()

        orig_refunds_fees_accrued = self.b.refundsFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(to_user)

        effective_amount = amount - swapFee if amount > swapFee else 0

        tx = self.b.reverseSwap(rid, to_user, origin_from, origin_tx_hash, amount, relay_eon, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - effective_amount
        assert self.b.refundsFeesAccrued() == orig_refunds_fees_accrued
        assert self.t.balanceOf(self.b) == orig_bridge_balance - effective_amount
        assert self.t.balanceOf(to_user) == orig_user_balance + effective_amount

        event = tx.events['ReverseSwap']
        assert event['rid'] == rid
        assert event['to'] == to_user
        assert brownie.convert.to_bytes(event['from'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [origin_from])
        assert brownie.convert.to_bytes(event['originTxHash'], 'bytes32') == origin_tx_hash
        assert event['effectiveAmount'] == effective_amount
        assert event['fee'] == swapFee

        return tx


@pytest.fixture(scope="module", autouse=True)
def tokenFactory(FetERC20Mock, accounts):
    def token_(test: BridgeTest = None) -> BridgeTest:
        test = test or BridgeTest()
        u = test.users
        t = test.token

        u.owner = accounts[0]
        u.relayer = accounts[1]
        u.users = accounts[2:]

        contract = FetERC20Mock.deploy("Fetch", "FET", t.totalSupply, t.tokenDecimals, {'from': u.owner})

        for user in u.users:
            contract.transfer(user, t.userFunds)

        test.t = contract
        print(f'ERC20 token contract')

        return test
    yield token_


@pytest.fixture(scope="module", autouse=True)
def bridgeFactory(Bridge, tokenFactory, accounts):
    def bridge_(test: BridgeTest = None) -> BridgeTest:
        test: BridgeTest = tokenFactory(test)
        b = test.bridge
        u = test.users

        contract = Bridge.deploy(
            test.t.address,
            b.cap,
            b.upperSwapLimit,
            b.lowerSwapLimit,
            b.swapFee,
            b.pausedSinceBlock,
            b.deleteProtectionPeriod,
            {'from': u.owner})

        pprint.pprint(contract.tx.events)

        b.deploymentBlockNumber = contract.tx.block_number
        b.pausedSinceBlockEffective = b.pausedSinceBlock if b.pausedSinceBlock > b.deploymentBlockNumber else b.deploymentBlockNumber
        contract.grantRole(u.relayerRole, u.relayer.address, {'from': u.owner})

        test.b = contract
        print(f'Bridge contract')

        return test
    yield bridge_


@pytest.fixture(autouse=True)
def isolate(fn_isolation):
    # perform a chain rewind after completing each test, to ensure proper isolation
    # https://eth-brownie.readthedocs.io/en/v1.10.3/tests-pytest-intro.html#isolation-fixtures
    pass


def test_initial_state(bridgeFactory):
    test: BridgeTest = bridgeFactory()

    print(f'{test.b.tx.events}')

    assert test.b.relayEon() == ((1<<64)-1)
    assert test.b.nextSwapId() == 0
    assert test.b.refundsFeesAccrued() == 0
    assert test.b.token() == test.t.address
    assert test.b.earliestDelete() == test.bridge.deploymentBlockNumber + test.bridge.deleteProtectionPeriod
    assert test.b.pausedSinceBlock() == test.bridge.pausedSinceBlockEffective
    assert test.b.refundsFeesAccrued() == 0


def test_newRelayEon_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    tx = test.b.newRelayEon({'from': test.users.relayer})
    assert test.b.relayEon() == 0
    evName = 'NewRelayEon'
    assert evName in tx.events
    assert tx.events[evName]['eon'] == 0


def test_swap_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    test.swap(user=test.users.users[0])


def test_reverseSwap_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    test.swap(user=user, amount=amount)
    test.revereseSwap(rid=0, to_user=user, amount=amount)


def test_refund_bacis(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)
    test.refund(id=swap_tx.events['Swap']['id'], to_user=user, amount=amount)


def test_refund_amount_smaller_than_fee(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.swapFee
    test.b.setLimits(test.bridge.upperSwapLimit, amount, amount)

    swap_tx = test.swap(user=user, amount=amount)

    test.b.setLimits(test.bridge.upperSwapLimit, amount+1, amount+1)

    tx = test.refund(id=swap_tx.events['Swap']['id'], to_user=user, amount=amount)
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == 0
    assert e['fee'] == amount


def test_refund_waive_fee(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)
    assert test.b.supply() == amount
    assert test.b.swapFee() > 0
    tx = test.refund(id=swap_tx.events['Swap']['id'], to_user=user, amount=amount, waive_fee=True)
    assert test.b.supply() == 0
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == amount
    assert e['fee'] == 0


def test_swap_reverts_when_bridge_is_paused(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.upperSwapLimit
    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.swap(user=user, amount=amount)

    test.b.pauseSince(0)
    with brownie.reverts(revert_msg="Contract has been paused"):
        test.b.swap(amount, test.vals.dest_swap_address, {'from': user})
