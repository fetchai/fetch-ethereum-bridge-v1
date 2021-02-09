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
    FeesWithdrawal = auto()
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
    delegate = None
    users = None
    adminRole: bytes = 0
    relayerRole: bytes = brownie.web3.solidityKeccak(['string'], ["RELAYER_ROLE"])
    delegateRole: bytes = brownie.web3.solidityKeccak(['string'], ["DELEGATE_ROLE"])

    notOwners = None
    notRelayers = None


@dataclass
class BridgeSetup:
    token: InitVar[TokenSetup]
    cap: int = None
    swapMax: int = None
    swapMin: int = None
    swapFee: int = None
    pausedSinceBlock: int = 0xffffffffffffffff
    pausedSinceBlockEffective: int = None
    deleteProtectionPeriod: int = 13
    earliestDelete: int = None
    deploymentBlockNumber: int = None

    def __post_init__(self, token):
        self.cap = token.toCanonical(1000)
        self.swapMax = token.toCanonical(100)
        self.swapMin = token.toCanonical(10)
        self.swapFee = token.toCanonical(5)


@dataclass
class ValuesSetup:
    bridge: InitVar[BridgeSetup]
    amount: int = None
    dest_swap_address = "some weird encoded and loooooonooooooooger than normal address"
    dest_swap_address_hash = brownie.web3.solidityKeccak(["string"], [dest_swap_address])
    src_tx_hash = brownie.web3.solidityKeccak(["string"], ["some tx has"])

    def __post_init__(self, bridge):
        self.amount = bridge.swapMin


@dataclass
class Setup__:
    users: UsersSetup = UsersSetup()
    token: TokenSetup = TokenSetup()
    bridge: BridgeSetup = None
    vals: ValuesSetup = None

    def __post_init__(self):
        self.bridge = BridgeSetup(self.token)
        self.vals = ValuesSetup(self.bridge)


@dataclass()
class BridgeTest:
    users: UsersSetup = UsersSetup()
    token: TokenSetup = TokenSetup()
    bridge: BridgeSetup = BridgeSetup(token)
    vals: ValuesSetup = ValuesSetup(bridge)
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

        event = tx.events[str(EventType.Swap)]
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

        orig_fees_accrued = self.b.feesAccrued()
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
        assert self.b.feesAccrued() == orig_fees_accrued + effective_fee
        assert self.b.refunds(id) == amount

        assert self.t.balanceOf(self.b) == orig_bridge_balance - refunded_amount
        assert self.t.balanceOf(to_user) == orig_user_balance + refunded_amount

        event = tx.events[str(EventType.SwapRefund)]
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

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(to_user)

        effective_amount = amount - swapFee if amount > swapFee else 0
        effective_fee = swapFee if amount > swapFee else amount

        tx = self.b.reverseSwap(rid, to_user, origin_from, origin_tx_hash, amount, relay_eon, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - effective_amount
        assert self.b.feesAccrued() == orig_fees_accrued + effective_fee
        assert self.t.balanceOf(self.b) == orig_bridge_balance - effective_amount
        assert self.t.balanceOf(to_user) == orig_user_balance + effective_amount

        event = tx.events[str(EventType.ReverseSwap)]
        assert event['rid'] == rid
        assert event['to'] == to_user
        assert brownie.convert.to_bytes(event['from'], 'bytes32') == brownie.web3.solidityKeccak(['string'], [origin_from])
        assert brownie.convert.to_bytes(event['originTxHash'], 'bytes32') == origin_tx_hash
        assert event['effectiveAmount'] == effective_amount
        assert event['fee'] == effective_fee

        return tx


    def withdrawExcessFunds(self,
                            target_address,
                            caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)

        expected_new_bridge_balance = orig_bridge_supply + orig_fees_accrued
        expected_excess_funds = orig_bridge_balance - expected_new_bridge_balance
        expected_new_target_address_balance = orig_target_address_balance + expected_excess_funds

        tx = self.b.withdrawExcessFunds(target_address, {'from': caller})

        assert self.b.supply() == orig_bridge_supply
        assert self.b.feesAccrued() == orig_fees_accrued
        assert self.t.balanceOf(self.b) == expected_new_bridge_balance
        assert self.t.balanceOf(target_address) == expected_new_target_address_balance

        event = tx.events[str(EventType.ExcessFundsWithdrawal)]
        assert event['targetAddress'] == target_address
        assert event['amount'] == expected_excess_funds

        return tx


    def mint(self,
             amount,
             caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)

        tx = self.b.mint(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply + amount
        assert self.b.feesAccrued() == orig_fees_accrued
        assert self.t.balanceOf(self.b) == orig_bridge_balance + amount

        event = tx.events['Transfer']
        assert event['from'] == "0x" + "00"*20
        assert event['to'] == self.b
        assert event['value'] == amount

        return tx


    def burn(self,
             amount,
             caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)

        tx = self.b.burn(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.feesAccrued() == orig_fees_accrued
        assert self.t.balanceOf(self.b) == orig_bridge_balance - amount

        event = tx.events['Transfer']
        assert event['from'] == self.b
        assert event['to'] == "0x" + "00"*20
        assert event['value'] == amount

        return tx


    def deposit(self,
                amount,
                caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_from_balance = self.t.balanceOf(caller)

        tx = self.b.deposit(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply + amount
        assert self.b.feesAccrued() == orig_fees_accrued
        assert self.t.balanceOf(self.b) == orig_bridge_balance + amount
        assert self.t.balanceOf(caller) == orig_from_balance - amount

        e_transfer = tx.events[str(EventType.Deposit)]
        assert e_transfer['fromAddress'] == caller
        assert e_transfer['amount'] == amount

        e_transfer = tx.events['Transfer']
        assert e_transfer['from'] == caller
        assert e_transfer['to'] == self.b
        assert e_transfer['value'] == amount

        return tx


    def withdraw(self,
                 target_address,
                 amount,
                 caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)

        tx = self.b.withdraw(target_address, amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.feesAccrued() == orig_fees_accrued
        assert self.t.balanceOf(self.b) == orig_bridge_balance - amount
        assert self.t.balanceOf(target_address) == orig_target_address_balance + amount

        e_transfer = tx.events[str(EventType.Withdraw)]
        assert e_transfer['targetAddress'] == target_address
        assert e_transfer['amount'] == amount

        e_transfer = tx.events['Transfer']
        assert e_transfer['from'] == self.b
        assert e_transfer['to'] == target_address
        assert e_transfer['value'] == amount

        return tx


    def withdrawFees(self,
                     target_address,
                     caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.feesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)

        tx = self.b.withdrawFees(target_address, {'from': caller})

        assert self.b.supply() == orig_bridge_supply
        assert self.b.feesAccrued() == 0
        assert self.t.balanceOf(self.b) == orig_bridge_balance - orig_fees_accrued
        assert self.t.balanceOf(target_address) == orig_target_address_balance + orig_fees_accrued

        e_transfer = tx.events[str(EventType.FeesWithdrawal)]
        assert e_transfer['targetAddress'] == target_address
        assert e_transfer['amount'] == orig_fees_accrued

        e_transfer = tx.events['Transfer']
        assert e_transfer['from'] == self.b
        assert e_transfer['to'] == target_address
        assert e_transfer['value'] == orig_fees_accrued

        return tx


    def deleteContract(self,
                       target_address,
                       caller = None):
        caller = caller or self.users.owner

        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)
        expected_resulting_target_address_balance = orig_target_address_balance + orig_bridge_balance

        tx = self.b.deleteContract(target_address, {'from': caller})

        assert self.t.balanceOf(self.b) == 0
        assert self.t.balanceOf(target_address) == expected_resulting_target_address_balance

        e_transfer = tx.events[str(EventType.DeleteContract)]
        assert e_transfer['targetAddress'] == target_address
        assert e_transfer['amount'] == orig_bridge_balance

        e_transfer = tx.events['Transfer']
        assert e_transfer['from'] == self.b
        assert e_transfer['to'] == target_address
        assert e_transfer['value'] == orig_bridge_balance

        return tx


    def pauseSince(self,
                   blockNumber,
                   caller = None):
        caller = caller or self.users.relayer

        tx = self.b.pauseSince(blockNumber, {'from': caller})

        effective_paused_since_block = tx.block_number if tx.block_number > blockNumber else blockNumber
        assert self.b.pausedSinceBlock() == effective_paused_since_block

        e = tx.events[str(EventType.Pause)]
        assert e['sinceBlock'] == effective_paused_since_block

        return tx


    def setLimits(self,
                  upper_swap_limit,
                  lower_swap_limit,
                  swap_fee,
                  caller = None):
        caller = caller or self.users.owner

        tx = self.b.setLimits(upper_swap_limit, lower_swap_limit, swap_fee, {'from': caller})

        assert upper_swap_limit == self.b.swapMax()
        assert lower_swap_limit == self.b.swapMin()
        assert swap_fee == self.b.swapFee()
        assert swap_fee <= lower_swap_limit <= upper_swap_limit

        e = tx.events[str(EventType.LimitsUpdate)]
        assert e['max'] == upper_swap_limit
        assert e['min'] == lower_swap_limit
        assert e['fee'] == swap_fee

        return tx


    def setCap(self,
               cap: int,
               caller = None):
        caller = caller or self.users.owner

        tx = self.b.setCap(cap, {'from': caller})

        assert cap == self.b.cap()

        e = tx.events[str(EventType.CapUpdate)]
        assert e['value'] == cap

        return tx


@pytest.fixture(scope="module", autouse=True)
def tokenFactory(FetERC20Mock, accounts):
    def token_(test: BridgeTest = None) -> BridgeTest:
        test = test or BridgeTest()
        u = test.users
        t = test.token

        u.owner = accounts[0]
        u.relayer = accounts[1]
        u.delegate = accounts[2]
        u.users = accounts[3:]
        u.notOwners = [u.relayer, u.delegate, u.users[0]]
        u.notRelayers = [u.owner, u.delegate, u.users[0]]

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
            b.swapMax,
            b.swapMin,
            b.swapFee,
            b.pausedSinceBlock,
            b.deleteProtectionPeriod,
            {'from': u.owner})

        pprint.pprint(contract.tx.events)

        b.deploymentBlockNumber = contract.tx.block_number
        b.pausedSinceBlockEffective = b.pausedSinceBlock if b.pausedSinceBlock > b.deploymentBlockNumber else b.deploymentBlockNumber
        b.earliestDelete = b.deploymentBlockNumber + b.deleteProtectionPeriod

        assert contract.pausedSinceBlock() == b.pausedSinceBlockEffective
        assert contract.earliestDelete() == b.earliestDelete

        contract.grantRole(u.relayerRole, u.relayer, {'from': u.owner})
        contract.grantRole(u.delegateRole, u.delegate, {'from': u.owner})

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
    assert test.b.feesAccrued() == 0
    assert test.b.token() == test.t.address
    assert test.b.earliestDelete() == test.bridge.deploymentBlockNumber + test.bridge.deleteProtectionPeriod
    assert test.b.pausedSinceBlock() == test.bridge.pausedSinceBlockEffective
    assert test.b.feesAccrued() == 0


def test_initial_state_non_trivial_pause_since_0(bridgeFactory):
    brownie.chain.mine(100)
    expectedPauseSince = brownie.web3.eth.blockNumber + 10000
    test = BridgeTest()
    test.bridge.pausedSinceBlock = expectedPauseSince

    test = bridgeFactory(test)
    assert test.bridge.deploymentBlockNumber < expectedPauseSince
    assert test.b.pausedSinceBlock() == expectedPauseSince


def test_initial_state_non_trivial_pause_since_1(bridgeFactory):
    n = 100
    brownie.chain.mine(n)
    test = BridgeTest()
    test.bridge.pausedSinceBlock = int(n / 2)

    test = bridgeFactory(test)
    assert int(n / 2) < test.b.pausedSinceBlock()
    assert test.b.pausedSinceBlock() == test.bridge.deploymentBlockNumber


def test_initial_state_non_trivial_earliest_delete(bridgeFactory):
    dpp = 10000
    n = 100
    brownie.chain.mine(n)
    test = BridgeTest()
    test.bridge.deleteProtectionPeriod = dpp
    test = bridgeFactory(test)

    assert test.b.earliestDelete() == test.bridge.deploymentBlockNumber + dpp


def test_newRelayEon_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Caller must be relayer"):
             test.b.newRelayEon({'from': u})

    tx = test.b.newRelayEon({'from': test.users.relayer})

    assert test.b.relayEon() == 0
    assert tx.events[str(EventType.NewRelayEon)]['eon'] == 0


def test_swap_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    test.swap(user=test.users.users[0])


def test_reverseSwap_basic(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    test.swap(user=user, amount=amount)

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Caller must be relayer"):
             test.revereseSwap(rid=0, to_user=user, amount=amount, caller=u)

    test.revereseSwap(rid=0, to_user=user, amount=amount)


def test_reverseSwap_amount_smaller_than_fee(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    test.swap(user=user, amount=amount)
    # PRECONDITION: shall pass - proof that reverse swap can be done
    test.revereseSwap(rid=0, to_user=user, amount=amount)

    test.swap(user=user, amount=amount)
    test.b.setLimits(test.bridge.swapMax, amount+1, amount+1)
    tx = test.revereseSwap(rid=1, to_user=user, amount=amount)
    # NOTE(pb): Just repeating basic check here again - this, all other possible consistency checks are done inside
    #           of the `test.revereseSwap(...)` method.
    e = tx.events[str(EventType.ReverseSwap)]
    assert e['effectiveAmount'] == 0
    assert e['fee'] == amount


def test_refund_bacis(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Caller must be relayer"):
             test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, caller=u)

    test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount)


def test_refund_amount_smaller_than_fee(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.swapFee
    test.b.setLimits(test.bridge.swapMax, amount, amount)

    swap_tx = test.swap(user=user, amount=amount)

    test.b.setLimits(test.bridge.swapMax, amount+1, amount+1)

    tx = test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount)
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == 0
    assert e['fee'] == amount


def test_refund_reverts_for_already_refunded_swap(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.vals.amount

    swap_tx1 = test.swap(user=user, amount=amount)
    test.swap(user=user, amount=amount)

    swap_id = swap_tx1.events[str(EventType.Swap)]['id']

    test.refund(id=swap_id, to_user=user, amount=amount)
    with brownie.reverts(revert_msg="Refund was already processed"):
        test.refund(id=swap_id, to_user=user, amount=amount)


def test_refund_reverts_for_invalid_id(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.vals.amount

    test.swap(user=user, amount=amount)
    swap_tx2 = test.swap(user=user, amount=amount)

    swap_id = swap_tx2.events[str(EventType.Swap)]['id']

    with brownie.reverts(revert_msg="Invalid swap id"):
        test.refund(id=swap_id + 1, to_user=user, amount=amount)


def test_refund_in_full(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)
    assert test.b.supply() == amount
    assert test.b.swapFee() > 0
    tx = test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)
    assert test.b.supply() == 0
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == amount
    assert e['fee'] == 0


def test_swap_reverts_when_bridge_is_paused(bridgeFactory):
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.swapMax
    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.swap(user=user, amount=amount)

    test.pauseSince(0)
    with brownie.reverts(revert_msg="Contract has been paused"):
        test.swap(user=user, amount=amount)


def test_paused_only_by_permitted_users(bridgeFactory):
    test = bridgeFactory()

    with brownie.reverts(revert_msg="Only relayer, admin or delegate"):
        test.pauseSince(0, caller=test.users.users[0])

    test.pauseSince(0, caller=test.users.relayer)
    test.pauseSince(1, caller=test.users.delegate)
    test.pauseSince(2, caller=test.users.owner)


def test_set_limits_basic(bridgeFactory):
    test = bridgeFactory()

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.setLimits(100, 10, 5, caller=u)

    test.setLimits(100, 10, 5)


def test_set_limits_reverts_on_comp_logic(bridgeFactory):
    test = bridgeFactory()

    permutations = [
        (100, 5, 10),
        (5, 100, 10),
        (5, 10, 100),
        (10, 5, 100)
    ]

    for max, min, fee in permutations:
        with brownie.reverts(revert_msg="fee<=lower<=upper violated"):
            test.setLimits(max, min, fee)


def test_set_cap_basic(bridgeFactory):
    test = bridgeFactory()

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.setCap(1000, caller=u)

    test.setCap(1000)


def test_set_limits_reverts_on_cap_violation(bridgeFactory):
    test: BridgeTest = bridgeFactory()

    orig_supply = test.b.supply()
    room = test.vals.amount
    test.setCap(orig_supply + room)
    amount = room + 1

    with brownie.reverts(revert_msg="Swap would exceed cap"):
        test.swap(user=test.users.users[0], amount=amount)

    with brownie.reverts(revert_msg="Deposit would exceed the cap"):
        test.deposit(amount)

    with brownie.reverts(revert_msg="Minting would exceed the cap"):
        test.mint(amount)


def test_withdraw_excess_funds_basic(bridgeFactory):
    excess_amount = 1234
    test = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.swapMax
    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.swap(user=user, amount=amount)

    test.t.transfer(test.b, excess_amount)
    tx = test.b.withdrawExcessFunds(test.users.users[1])
    # Note(pb): Following assertion is not necessary, it is done inside of the above call
    #  `BridgeTest.withdrawExcessFunds(...)`
    e = tx.events[str(EventType.ExcessFundsWithdrawal)]
    assert e['targetAddress'] == test.users.users[1]
    assert e['amount'] == excess_amount


def test_withdraw_excess_funds_with_refunds_fees(bridgeFactory):
    excess_amount = 1234
    test = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.swapMax
    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    tx3 = test.swap(user=user, amount=amount)

    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    test.refund(id=tx3.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    assert test.b.feesAccrued() > 0

    test.t.transfer(test.b, excess_amount)

    tx = test.b.withdrawExcessFunds(test.users.users[1])
    # Note(pb): Following assertion is not necessary, it is done inside of the above call
    #  `BridgeTest.withdrawExcessFunds(...)`
    e = tx.events[str(EventType.ExcessFundsWithdrawal)]
    assert e['targetAddress'] == test.users.users[1]
    assert e['amount'] == excess_amount


def test_refunds_fees(bridgeFactory):
    excess_amount = 1234
    test = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.swapMax

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    # PRECONDITION: Add some supply, refunded fees, and excess funds:
    test.t.transfer(test.b, excess_amount)
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    tx3 = test.swap(user=user, amount=amount)

    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    test.refund(id=tx3.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    assert test.b.feesAccrued() == orig_refunds_fees + 2*test.b.swapFee()
    assert test.b.supply() == orig_supply + amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + 2*test.b.swapFee() + excess_amount


def test_mint(bridgeFactory):
    mint_amount = 972
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.swapMax

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    tx = test.mint(mint_amount)

    assert test.b.feesAccrued() == orig_refunds_fees + test.b.swapFee()
    assert test.b.supply() == orig_supply + amount + mint_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + test.b.swapFee() + mint_amount + excess_amount


def test_burn(bridgeFactory):
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.swapMax

    burn_amount = amount/2
    assert burn_amount > 0

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.burn(burn_amount, caller=u)

    test.burn(burn_amount)

    assert test.b.feesAccrued() == orig_refunds_fees + test.b.swapFee()
    assert test.b.supply() == orig_supply + amount - burn_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + test.b.swapFee() - burn_amount + excess_amount


def test_deposit(bridgeFactory):
    deposit_amount = 972
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    from_user = test.users.owner
    amount = test.bridge.swapMax

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_from_user_balance = test.t.balanceOf(from_user)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    test.t.approve(test.b, deposit_amount, {'from': from_user})

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            tx = test.deposit(deposit_amount, caller=u)

    tx = test.deposit(deposit_amount)

    assert test.b.feesAccrued() == orig_refunds_fees + test.b.swapFee()
    assert test.b.supply() == orig_supply + amount + deposit_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + test.b.swapFee() + deposit_amount + excess_amount
    assert test.t.balanceOf(from_user) == orig_from_user_balance - deposit_amount


def test_withdraw(bridgeFactory):
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.swapMax
    withdraw_amount = 972

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.withdraw(target_to_user, withdraw_amount, caller=u)

    tx = test.withdraw(target_to_user, withdraw_amount)

    assert test.b.feesAccrued() == orig_refunds_fees + test.b.swapFee()
    assert test.b.supply() == orig_supply + amount - withdraw_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + test.b.swapFee() - withdraw_amount + excess_amount
    assert test.t.balanceOf(target_to_user) == orig_target_to_user_balance + withdraw_amount


def test_withdraw_refunds_fees(bridgeFactory):
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.swapMax

    orig_supply = test.b.supply()
    orig_refunds_fees = test.b.feesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)
    expected_refunds_fees_accrued = orig_refunds_fees + test.b.swapFee()

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    assert test.b.feesAccrued() == expected_refunds_fees_accrued
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + excess_amount + test.b.swapFee()

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.withdrawFees(target_to_user, caller=u)

    test.withdrawFees(target_to_user)

    assert test.b.feesAccrued() == 0
    assert test.b.supply() == orig_supply + amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + excess_amount - orig_refunds_fees
    assert test.t.balanceOf(target_to_user) == orig_target_to_user_balance + expected_refunds_fees_accrued


def test_delete_contract_badsic(bridgeFactory):
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.swapMax

    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    expected_resulting_target_to_user_balance = orig_target_to_user_balance + orig_contract_balance + amount + excess_amount + test.b.swapFee()

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Caller must be owner"):
            test.deleteContract(target_to_user, caller=u)

    test.deleteContract(target_to_user)

    assert test.t.balanceOf(test.b) == 0
    assert test.t.balanceOf(target_to_user) == expected_resulting_target_to_user_balance


def test_delete_contract_badsic1(bridgeFactory):
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.swapMax

    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)

    # PRECONDITION: shall pass, to prove that the Bridge contract is *not* paused yet
    test.t.transfer(test.b, excess_amount, {'from': user})
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    expected_resulting_target_to_user_balance = orig_target_to_user_balance + orig_contract_balance + amount + excess_amount + test.b.swapFee()

    for u in test.users.notOwners:
         with brownie.reverts(revert_msg="Caller must be owner"):
             test.deleteContract(target_to_user, caller=u)

    test.deleteContract(target_to_user)

    assert test.t.balanceOf(test.b) == 0
    assert test.t.balanceOf(target_to_user) == expected_resulting_target_to_user_balance
