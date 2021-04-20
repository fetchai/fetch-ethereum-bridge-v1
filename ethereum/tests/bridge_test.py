#!/usr/bin/env python3

import pytest
import brownie
from brownie import FetERC20Mock, Bridge, accounts
from brownie.network.account import _PrivateKeyAccount as Account, Accounts
from dataclasses import dataclass, InitVar, field
from enum import Enum, auto
from typing import List
from scripts.deployment_manifest_schema import (
    BridgeConstructorParams,
    FetERC20MockConstructorParams
    )


CanonicalFET = int


class AutoNameEnum(Enum):
    def _generate_next_value_(name, start, count, last_values):
        return name

    def __str__(self):
        return self.value.split('.')[-1]


class EventType(AutoNameEnum):
    Swap = auto()
    SwapRefund = auto()
    ReverseSwap = auto()
    PausePublicApi = auto()
    PauseRelayerApi = auto()
    NewRelayEon = auto()
    SwapLimitsUpdate = auto()
    ReverseSwapLimitsUpdate = auto()
    CapUpdate = auto()
    ReverseAggregatedAllowanceUpdate = auto()
    ReverseAggregatedAllowanceApproverCapUpdate = auto()
    Withdraw = auto()
    Deposit = auto()
    FeesWithdrawal = auto()
    ExcessFundsWithdrawal = auto()
    DeleteContract = auto()


@dataclass
class TokenSetup(FetERC20MockConstructorParams):
    multiplier: int
    userFunds: int

    @classmethod
    def toCanonicalCls(cls, amount_fet: int, multiplier: int) -> CanonicalFET:
        return amount_fet * multiplier

    def toCanonical(self, amount_fet: int) -> CanonicalFET:
        return self.toCanonicalCls(amount_fet, self.multiplier)

    @classmethod
    def default(cls):
        decimals_ = 18
        multiplier = 10**decimals_
        return TokenSetup(
            name="Fetch",
            symbol="FET",
            initialSupply = cls.toCanonicalCls(1000000, multiplier=multiplier),
            decimals_=decimals_,
            multiplier=multiplier,
            userFunds=cls.toCanonicalCls(1000, multiplier=multiplier)
            )


@dataclass
class UsersSetup:
    owner: Account = None
    relayer: Account = None
    approver: Account = None
    monitor: Account = None
    users: List[Account] = None
    everyone: List[Account] = None

    adminRole: bytes = None
    relayerRole: bytes = None
    approverRole: bytes = None
    monitorRole: bytes = None

    canPauseUsers: List[Account] = None
    canNOTPauseUsers: List[Account] = None

    canUnpauseUsers: List[Account] = None
    canNOTUnpauseUsers: List[Account] = None

    canSetReverseAggregatedAllowance: List[Account] = None
    canNOTSetReverseAggregatedAllowance: List[Account] = None

    canSetReverseAggregatedAllowance: List[Account] = None
    canNOTSetReverseAggregatedAllowance: List[Account] = None

    notOwners: List[Account] = None
    notRelayers: List[Account] = None
    notMonitors: List[Account] = None
    notApprovers: List[Account] = None

    def __post_init__(self):
        self.relayerRole = brownie.web3.solidityKeccak(['string'], ["RELAYER_ROLE"])
        self.approverRole = brownie.web3.solidityKeccak(['string'], ["APPROVER_ROLE"])
        self.monitorRole  = brownie.web3.solidityKeccak(['string'], ["MONITOR_ROLE"])

    @classmethod
    def default(cls, accounts: Accounts):
        owner = accounts[0]
        relayer = accounts[1]
        approver = accounts[2]
        monitor = accounts[3]
        users = accounts[4:]
        everyone = accounts[0:5]

        canPauseUsers = [owner, monitor]
        canUnpauseUsers = [owner]
        canSetReverseAggregatedAllowance = [owner, approver]

        return UsersSetup(
            owner=owner,
            relayer=relayer,
            approver=approver,
            monitor=monitor,
            users=users,
            everyone=everyone,
            canPauseUsers=canPauseUsers,
            canNOTPauseUsers=list(set(everyone) - set(canPauseUsers)),
            canUnpauseUsers=canUnpauseUsers,
            canNOTUnpauseUsers=list(set(everyone) - set(canUnpauseUsers)),
            canSetReverseAggregatedAllowance=canSetReverseAggregatedAllowance,
            canNOTSetReverseAggregatedAllowance=list(set(everyone) - set(canSetReverseAggregatedAllowance)),
            notOwners=list(set(everyone) - {owner}),
            notRelayers=list(set(everyone) - {relayer}),
            notApprovers=list(set(everyone) - {approver}),
            notMonitors=list(set(everyone) - {monitor}),
            )


@dataclass
class BridgeSetup(BridgeConstructorParams):
    pausedSinceBlockPublicApiEffective: int = None
    pausedSinceBlockRelayerApiEffective: int = None
    earliestDelete: int = None
    deploymentBlockNumber: int = None

    @classmethod
    def default(self, token: TokenSetup):
        return BridgeSetup(
            ERC20Address="",
            cap=token.toCanonical(1000),
            reverseAggregatedAllowance=token.toCanonical(1000),
            reverseAggregatedAllowanceApproverCap=token.toCanonical(2000),
            swapMax=token.toCanonical(101),
            swapMin=token.toCanonical(11),
            reverseSwapMax = token.toCanonical(100),
            reverseSwapMin = token.toCanonical(10),
            reverseSwapFee = token.toCanonical(5),
            pausedSinceBlockPublicApi=0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff,
            pausedSinceBlockRelayerApi=0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff,
            deleteProtectionPeriod=13
            )


@dataclass
class ValuesSetup:
    amount: int = None
    dest_swap_address: str = None
    dest_swap_address_hash: bytes = None
    src_tx_hash: bytes = None

    @classmethod
    def default(cls, bridge: BridgeSetup):
        dest_swap_address="some weird encoded and loooooonooooooooger than normal address"
        return ValuesSetup(
            amount=max(bridge.reverseSwapMin, bridge.swapMin),
            dest_swap_address=dest_swap_address,
            dest_swap_address_hash=brownie.web3.solidityKeccak(["string"], [dest_swap_address]),
            src_tx_hash=brownie.web3.solidityKeccak(["string"], ["some tx has"])
            )


@dataclass
class Setup__:
    users: UsersSetup = None
    token: TokenSetup = None
    bridge: BridgeSetup = None
    vals: ValuesSetup = None

    @classmethod
    def default(self, accounts: Accounts):
        return Setup__(
            users=UsersSetup.default(accounts),
            token=TokenSetup.default(),
            bridge=BridgeSetup.default(self.token),
            vals=ValuesSetup.default(self.bridge)
            )


@dataclass
class BridgeTest:
    accounts: InitVar[Accounts]
    users: UsersSetup = None # field(default_factory=UsersSetup)
    token: TokenSetup = None # field(default_factory=TokenSetup)
    bridge: BridgeSetup = None # field(default_factory=lambda: BridgeSetup(BridgeTest.token))
    vals: ValuesSetup = None # field(default_factory=lambda: ValuesSetup(BridgeTest.bridge))
    t: FetERC20Mock = None
    b: Bridge = None

    def __post_init__(self, accounts: Accounts):
        self.users = UsersSetup.default(accounts)
        self.token = TokenSetup.default()
        self.bridge = BridgeSetup.default(self.token)
        self.vals = ValuesSetup.default(self.bridge)

    def standard_setup(self,
                       user=None,
                       amount=None,
                       excess_amount=1234,
                       relay_eon=None,
                       caller=None):
        """
        This method is for creating standard initial setup for tests.
        It's goal is to set contract state the way that all state variables
        related to keeping track of contract's financial affairs and operations
        will be set to non-trivial(non-default) values.
        """

        user = user or self.users.users[0]

        # Add excess funds
        self.t.transfer(self.b, excess_amount, {'from': user})
        # Add 3 swaps
        self.swap(user=user, amount=amount)
        tx2 = self.swap(user=user, amount=amount)
        self.swap(user=user, amount=amount)
        # Refund 2nd swap
        self.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False, caller=caller)
        # Add reverse swap
        self.reverseSwap(rid=0, to_user=user, amount=amount, relay_eon=relay_eon, caller=caller)


    def newRelayEon(self, caller: Account = None):
        caller = caller or self.users.relayer

        orig_relay_eon = self.b.getRelayEon()

        if orig_relay_eon == ((1<<64) - 1):
            expected_new_relay_eon = 0
        else:
            expected_new_relay_eon = orig_relay_eon + 1

        tx = self.b.newRelayEon({'from': caller})

        assert expected_new_relay_eon == self.b.getRelayEon()
        assert tx.events[str(EventType.NewRelayEon)]['eon'] == expected_new_relay_eon

        return tx


    def swap(self, user, amount: int = None, dest_addr: str = None):
        amount = self.vals.amount if amount is None else amount
        dest_addr = self.vals.dest_swap_address if dest_addr is None else dest_addr

        origSwapId = self.b.nextSwapId()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(user)
        
        # PRECONDITION:
        self.t.approve(self.b, amount, {'from': user})
        assert self.t.allowance(user, self.b) == amount

        tx = self.b.swap(amount, dest_addr, {'from': user})

        assert self.b.nextSwapId() == origSwapId + 1
        assert self.b.supply() == orig_bridge_supply + amount
        assert self.t.balanceOf(self.b) == orig_bridge_balance + amount
        assert self.t.balanceOf(user) == orig_user_balance - amount

        event = tx.events[str(EventType.Swap)]
        assert event['id'] == origSwapId
        assert event['from'] == user
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
        reverseSwapFee = 0 if waive_fee else self.b.reverseSwapFee()

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(to_user)

        effective_fee = min(amount, reverseSwapFee)
        refunded_amount = amount - effective_fee

        #assert self.b.refunds(id) == 0
        if waive_fee:
            tx = self.b.refundInFull(id, to_user, amount, relay_eon, {'from': caller})
        else:
            tx = self.b.refund(id, to_user, amount, relay_eon, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.getFeesAccrued() == orig_fees_accrued + effective_fee
        assert self.b.refunds(id) == amount

        assert self.t.balanceOf(self.b) == orig_bridge_balance - refunded_amount
        assert self.t.balanceOf(to_user) == orig_user_balance + refunded_amount

        event = tx.events[str(EventType.SwapRefund)]
        assert event['id'] == id
        assert event['to'] == to_user
        assert event['refundedAmount'] == refunded_amount
        assert event['fee'] == effective_fee

        return tx


    def reverseSwap(self,
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
        reverseSwapFee = self.b.reverseSwapFee()

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_user_balance = self.t.balanceOf(to_user)

        effective_amount = amount - reverseSwapFee if amount > reverseSwapFee else 0
        effective_fee = min(amount, reverseSwapFee)

        tx = self.b.reverseSwap(rid, to_user, origin_from, origin_tx_hash, amount, relay_eon, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.getFeesAccrued() == orig_fees_accrued + effective_fee
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


    def mint(self,
             amount,
             caller = None):
        caller = caller or self.users.owner

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)

        tx = self.b.mint(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply + amount
        assert self.b.getFeesAccrued() == orig_fees_accrued
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

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)

        tx = self.b.burn(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.getFeesAccrued() == orig_fees_accrued
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

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_from_balance = self.t.balanceOf(caller)

        tx = self.b.deposit(amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply + amount
        assert self.b.getFeesAccrued() == orig_fees_accrued
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

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)

        tx = self.b.withdraw(target_address, amount, {'from': caller})

        assert self.b.supply() == orig_bridge_supply - amount
        assert self.b.getFeesAccrued() == orig_fees_accrued
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

        orig_fees_accrued = self.b.getFeesAccrued()
        orig_bridge_supply = self.b.supply()
        orig_bridge_balance = self.t.balanceOf(self.b)
        orig_target_address_balance = self.t.balanceOf(target_address)

        tx = self.b.withdrawFees(target_address, {'from': caller})

        assert self.b.supply() == orig_bridge_supply
        assert self.b.getFeesAccrued() == 0
        resul_contract_balance = self.t.balanceOf(self.b)
        assert resul_contract_balance == orig_bridge_balance - orig_fees_accrued
        assert resul_contract_balance == orig_bridge_supply
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
        contract_address = self.b.address

        tx = self.b.deleteContract(target_address, {'from': caller})

        assert self.t.balanceOf(contract_address) == 0
        assert self.t.balanceOf(target_address) == expected_resulting_target_address_balance

        e_transfer = tx.events[str(EventType.DeleteContract)]
        assert e_transfer['targetAddress'] == target_address
        assert e_transfer['amount'] == orig_bridge_balance

        e_transfer = tx.events['Transfer']
        assert e_transfer['from'] == contract_address
        assert e_transfer['to'] == target_address
        assert e_transfer['value'] == orig_bridge_balance

        return tx


    def pausePublicApiSince(self,
                            blockNumber: int,
                            caller: Account = None):
        caller = caller or self.users.owner

        tx = self.b.pausePublicApiSince(blockNumber, {'from': caller})

        effective_paused_since_block = tx.block_number if tx.block_number > blockNumber else blockNumber
        assert self.b.pausedSinceBlockPublicApi() == effective_paused_since_block
        assert self.b.pausedSinceBlockPublicApi() == self.b.getPausedSinceBlockPublicApi()

        e = tx.events[str(EventType.PausePublicApi)]
        assert e['sinceBlock'] == effective_paused_since_block

        return tx


    def pauseRelayerApiSince(self,
                             blockNumber: int,
                             caller: Account = None):
        caller = caller or self.users.owner

        tx = self.b.pauseRelayerApiSince(blockNumber, {'from': caller})

        effective_paused_since_block = tx.block_number if tx.block_number > blockNumber else blockNumber
        assert self.b.pausedSinceBlockRelayerApi() == effective_paused_since_block
        assert self.b.pausedSinceBlockRelayerApi() == self.b.getPausedSinceBlockRelayerApi()

        e = tx.events[str(EventType.PauseRelayerApi)]
        assert e['sinceBlock'] == effective_paused_since_block

        return tx

    def setSwapLimits(self,
                      swap_max,
                      swap_min,
                      caller = None):
        caller = caller or self.users.owner

        tx = self.b.setSwapLimits(swap_max, swap_min, {'from': caller})

        assert swap_max == self.b.getSwapMax()
        assert swap_min == self.b.getSwapMin()

        e = tx.events[str(EventType.SwapLimitsUpdate)]
        assert e['max'] == swap_max
        assert e['min'] == swap_min

    def setReverseSwapLimits(self,
                  swap_max,
                  swap_min,
                  swap_fee,
                  caller = None):
        caller = caller or self.users.owner

        tx = self.b.setReverseSwapLimits(swap_max, swap_min, swap_fee, {'from': caller})

        assert swap_max == self.b.getReverseSwapMax()
        assert swap_min == self.b.getReverseSwapMin()
        assert swap_fee == self.b.getReverseSwapFee()
        assert swap_fee <= swap_min <= swap_max

        e = tx.events[str(EventType.ReverseSwapLimitsUpdate)]
        assert e['max'] == swap_max
        assert e['min'] == swap_min
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

    def setReverseAggregatedAllowance(self,
               allowance: int,
               caller = None):
        caller = caller or self.users.owner

        tx = self.b.setReverseAggregatedAllowance(allowance, {'from': caller})

        assert allowance == self.b.getReverseAggregatedAllowance()

        e = tx.events[str(EventType.ReverseAggregatedAllowanceUpdate)]
        assert e['value'] == allowance

        return tx

    def setReverseAggregatedAllowanceApproverCap(self,
                                                 cap: int,
                                                 caller = None):
        caller = caller or self.users.owner

        tx = self.b.setReverseAggregatedAllowanceApproverCap(cap, {'from': caller})

        assert cap == self.b.getReverseAggregatedAllowanceApproverCap()

        e = tx.events[str(EventType.ReverseAggregatedAllowanceApproverCapUpdate)]
        assert cap == e['value']

        return tx


@pytest.fixture(scope="module", autouse=True)
def tokenFactory(FetERC20Mock, accounts):
    def token_(test: BridgeTest = None) -> BridgeTest:
        test = test or BridgeTest(accounts=accounts)
        u = test.users
        t = test.token

        contract = FetERC20Mock.deploy("Fetch", "FET", t.initialSupply, t.decimals_, {'from': u.owner})

        for user in u.users:
            contract.transfer(user, t.userFunds)

        test.t = contract

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
            b.reverseAggregatedAllowance,
            b.reverseAggregatedAllowanceApproverCap,
            b.swapMax,
            b.swapMin,
            b.reverseSwapMax,
            b.reverseSwapMin,
            b.reverseSwapFee,
            b.pausedSinceBlockPublicApi,
            b.pausedSinceBlockRelayerApi,
            b.deleteProtectionPeriod,
            {'from': u.owner})

        b.deploymentBlockNumber = contract.tx.block_number
        b.pausedSinceBlockPublicApiEffective = b.pausedSinceBlockPublicApi if b.pausedSinceBlockPublicApi > b.deploymentBlockNumber else b.deploymentBlockNumber
        b.pausedSinceBlockRelayerApiEffective = b.pausedSinceBlockRelayerApi if b.pausedSinceBlockRelayerApi > b.deploymentBlockNumber else b.deploymentBlockNumber
        b.earliestDelete = b.deploymentBlockNumber + b.deleteProtectionPeriod

        assert contract.pausedSinceBlockPublicApi() == b.pausedSinceBlockPublicApiEffective
        assert contract.pausedSinceBlockRelayerApi() == b.pausedSinceBlockRelayerApiEffective
        assert contract.earliestDelete() == b.earliestDelete

        contract.grantRole(u.relayerRole, u.relayer, {'from': u.owner})
        contract.grantRole(u.approverRole, u.approver, {'from': u.owner})
        contract.grantRole(u.monitorRole, u.monitor, {'from': u.owner})

        test.b = contract

        return test

    yield bridge_


@pytest.fixture(autouse=True)
def isolate(fn_isolation):
    # perform a chain rewind after completing each test, to ensure proper isolation
    # https://eth-brownie.readthedocs.io/en/v1.10.3/tests-pytest-intro.html#isolation-fixtures
    pass


def test_initial_state(bridgeFactory):
    test: BridgeTest = bridgeFactory()
    assert test.b.getCap() == test.bridge.cap
    assert test.b.getReverseSwapMax() == test.bridge.reverseSwapMax
    assert test.b.getReverseSwapMin() == test.bridge.reverseSwapMin
    assert test.b.getReverseSwapFee() == test.bridge.reverseSwapFee
    assert test.b.getReverseAggregatedAllowance() == test.bridge.reverseAggregatedAllowance
    assert test.b.getReverseAggregatedAllowanceApproverCap() == test.bridge.reverseAggregatedAllowanceApproverCap
    assert test.b.getRelayEon() == ((1<<64)-1)
    assert test.b.getNextSwapId() == 0
    assert test.b.getFeesAccrued() == 0
    assert test.b.getToken() == test.t.address
    assert test.b.getEarliestDelete() == test.bridge.deploymentBlockNumber + test.bridge.deleteProtectionPeriod
    assert test.b.getPausedSinceBlockPublicApi() == test.bridge.pausedSinceBlockPublicApiEffective
    assert test.b.getPausedSinceBlockRelayerApi() == test.bridge.pausedSinceBlockRelayerApiEffective
    assert test.b.getFeesAccrued() == 0


def test_initial_state_non_trivial_pause_since_0(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    brownie.chain.mine(100)
    expectedPauseSincePublicApi = brownie.web3.eth.blockNumber + 10000
    expectedPauseSinceRelayerApi = brownie.web3.eth.blockNumber + 10001
    test = BridgeTest(accounts=accounts)
    test.bridge.pausedSinceBlockPublicApi = expectedPauseSincePublicApi
    test.bridge.pausedSinceBlockRelayerApi = expectedPauseSinceRelayerApi

    # ===   WHEN / TEST SUBJECT  ==========================
    test = bridgeFactory(test)

    # ===   THEN / VERIFICATION:  =========================
    assert test.bridge.deploymentBlockNumber < expectedPauseSincePublicApi
    assert test.b.getPausedSinceBlockPublicApi() == expectedPauseSincePublicApi
    assert test.b.getPausedSinceBlockRelayerApi() == expectedPauseSinceRelayerApi


def test_initial_state_non_trivial_pause_since_1(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    n = 100
    pausedSinceBlockPublicApi = int(n / 2)
    pausedSinceBlockRelayerApi = int(n / 2) + 1

    brownie.chain.mine(n)
    test = BridgeTest(accounts=accounts)
    test.bridge.pausedSinceBlockPublicApi = pausedSinceBlockPublicApi
    test.bridge.pausedSinceBlockRelayerApi = pausedSinceBlockRelayerApi

    # ===   WHEN / TEST SUBJECT  ==========================
    test = bridgeFactory(test)

    # ===   THEN / VERIFICATION:  =========================
    assert pausedSinceBlockPublicApi < test.b.getPausedSinceBlockPublicApi()
    assert pausedSinceBlockRelayerApi < test.b.getPausedSinceBlockRelayerApi()
    assert test.b.getPausedSinceBlockPublicApi() == test.bridge.deploymentBlockNumber
    assert test.b.getPausedSinceBlockRelayerApi() == test.bridge.deploymentBlockNumber


def test_initial_state_non_trivial_earliest_delete(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    dpp = 10000
    n = 100
    brownie.chain.mine(n)
    test = BridgeTest(accounts)
    test.bridge.deleteProtectionPeriod = dpp

    # ===   WHEN / TEST SUBJECT  ==========================
    test = bridgeFactory(test)

    # ===   THEN / VERIFICATION:  =========================
    assert test.b.earliestDelete() == test.bridge.deploymentBlockNumber + dpp


def test_newRelayEon_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Only relayer role"):
             test.newRelayEon(caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    tx = test.newRelayEon(caller=test.users.relayer)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    assert test.b.getRelayEon() == 0
    assert tx.events[str(EventType.NewRelayEon)]['eon'] == 0


def test_swap_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()

    # ===   WHEN / TEST SUBJECT  ==========================
    test.swap(user=test.users.users[0])

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_reverseSwap_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    test.swap(user=user, amount=amount)

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Only relayer role"):
             test.reverseSwap(rid=0, to_user=user, amount=amount, caller=u)

    # ===   GIVEN / PRECONDITIONS:  =======================
    test.reverseSwap(rid=0, to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_reverseSwap_amount_smaller_than_fee(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    bridgeFactory(test)
    user = test.users.users[0]
    reverseSwapFee = test.b.reverseSwapFee()
    amount = test.bridge.reverseSwapMin + reverseSwapFee

    amount_smaller_than_fee = reverseSwapFee - 1
    assert amount_smaller_than_fee > 1

    # Proof that it is possible to execute swap & reverseSwap successfully:
    test.swap(user=user, amount=amount)
    test.reverseSwap(rid=0, to_user=user, amount=amount)

    # Adding supply to the contract (there is none left in the contract after above 2 proof-calls):
    test.swap(user=user, amount=amount)

    # ===   WHEN / TEST SUBJECT  ==========================
    tx = test.reverseSwap(rid=1, to_user=user, amount=amount_smaller_than_fee)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again - this, all other possible consistency checks are done inside of the
    # `test.reverseSwap(...)` method.
    e = tx.events[str(EventType.ReverseSwap)]
    assert e['effectiveAmount'] == 0
    assert e['fee'] == amount_smaller_than_fee


def test_refund_bacis(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)

    for u in test.users.notRelayers:
         with brownie.reverts(revert_msg="Only relayer role"):
             test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_refund_amount_smaller_than_fee(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.reverseSwapFee
    test.b.setSwapLimits(test.bridge.reverseSwapMax, amount)

    swap_tx = test.swap(user=user, amount=amount)

    test.b.setReverseSwapLimits(test.bridge.reverseSwapMax, amount+1, amount+1)

    # ===   WHEN / TEST SUBJECT  ==========================
    tx = test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == 0
    assert e['fee'] == amount


def test_refund_reverts_for_already_refunded_swap(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.vals.amount

    swap_tx1 = test.swap(user=user, amount=amount)
    test.swap(user=user, amount=amount)

    swap_id = swap_tx1.events[str(EventType.Swap)]['id']

    test.refund(id=swap_id, to_user=user, amount=amount)

    # ===   WHEN / TEST SUBJECT  ==========================
    with brownie.reverts(revert_msg="Refund was already processed"):
        test.refund(id=swap_id, to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_refund_reverts_for_invalid_id(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.vals.amount

    test.swap(user=user, amount=amount)
    swap_tx2 = test.swap(user=user, amount=amount)

    swap_id = swap_tx2.events[str(EventType.Swap)]['id']

    # ===   WHEN / TEST SUBJECT  ==========================
    with brownie.reverts(revert_msg="Invalid swap id"):
        test.refund(id=swap_id + 1, to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_refund_in_full(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.vals.amount
    swap_tx = test.swap(user=user, amount=amount)
    assert test.b.supply() == amount
    assert test.b.reverseSwapFee() > 0

    # ===   WHEN / TEST SUBJECT  ==========================
    tx = test.refund(id=swap_tx.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.supply() == 0
    e = tx.events[str(EventType.SwapRefund)]
    assert e['refundedAmount'] == amount
    assert e['fee'] == 0


def test_swap_reverts_when_public_api_is_paused(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.reverseSwapMax
    test.swap(user=user, amount=amount)

    test.pausePublicApiSince(0, caller=test.users.monitor)

    # ===   WHEN / TEST SUBJECT  ==========================
    for u in test.users.everyone:
        with brownie.reverts(revert_msg="Contract has been paused"):
            test.swap(user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_swap_successful_when_relayer_api_is_paused(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    user = test.users.users[0]
    amount = test.bridge.reverseSwapMax

    test.pauseRelayerApiSince(0, caller=test.users.monitor)

    # ===   WHEN / TEST SUBJECT  ==========================
    for u in test.users.everyone:
        test.swap(user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_pausing_public_api_only_by_permitted_users(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    # First proving negative
    for u in test.users.canNOTPauseUsers:
        with brownie.reverts(revert_msg="Only admin or monitor role"):
            test.pausePublicApiSince(0, caller=test.users.users[0])

    # ===   WHEN / TEST SUBJECT  ==========================
    # Test positive
    for i, user in enumerate(test.users.canPauseUsers, start=0):
        test.pausePublicApiSince(i, caller=user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_pausing_relayer_api_only_by_permitted_users(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    # First proving negative
    for u in test.users.canNOTPauseUsers:
        with brownie.reverts(revert_msg="Only admin or monitor role"):
            test.pauseRelayerApiSince(0, caller=test.users.users[0])

    # ===   WHEN / TEST SUBJECT  ==========================
    # Test positive
    for i, user in enumerate(test.users.canPauseUsers, start=0):
        test.pauseRelayerApiSince(i, caller=user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_unpausing_public_api_only_by_permitted_users(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    # First proving negative
    for u in test.users.canNOTUnpauseUsers:
        with brownie.reverts(revert_msg="Only admin role"):
            test.pausePublicApiSince(brownie.web3.eth.blockNumber + 100, caller=test.users.users[0])

    # ===   WHEN / TEST SUBJECT  ==========================
    # Test positive
    for user in test.users.canUnpauseUsers:
        test.pausePublicApiSince(brownie.web3.eth.blockNumber + 100, caller=user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_unpausing_relayer_api_only_by_permitted_users(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    # First proving negative
    for u in test.users.canNOTUnpauseUsers:
        with brownie.reverts(revert_msg="Only admin role"):
            test.pauseRelayerApiSince(brownie.web3.eth.blockNumber + 100, caller=test.users.users[0])

    # ===   WHEN / TEST SUBJECT  ==========================
    # Test positive
    for user in test.users.canUnpauseUsers:
        test.pauseRelayerApiSince(brownie.web3.eth.blockNumber + 100, caller=user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_relayer_api_reverts_when_relayer_api_is_paused(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    test.standard_setup()

    user = test.users.users[0]
    amount = test.bridge.reverseSwapMin + test.bridge.reverseSwapFee

    # Adding 6 swaps to bump up supply and to prepare for refund & reverseSwap calls
    txs = [test.swap(user=user, amount=amount) for _ in range(0,6)]

    # Prove negative (relative to test objective)
    test.refund(id=txs[0].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    test.refund(id=txs[1].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)
    test.reverseSwap(rid=0, to_user=user, amount=amount)
    test.newRelayEon()

    test.pauseRelayerApiSince(0)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Contract has been paused"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.refund(id=txs[0].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Contract has been paused"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.refund(id=txs[1].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Contract has been paused"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.reverseSwap(rid=0, to_user=user, amount=amount)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Contract has been paused"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.newRelayEon()


def test_set_reverse_swap_limits_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    orig_swap_max = test.b.getSwapMax()
    orig_swap_min = test.b.getSwapMin()

    new_swap_max = test.b.getReverseSwapMax() + 1
    new_swap_min = test.b.getReverseSwapMin() + 1
    new_swap_fee = test.b.getReverseSwapFee() + 1

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.setReverseSwapLimits(new_swap_max, new_swap_min, new_swap_fee, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.setReverseSwapLimits(new_swap_max, new_swap_min, new_swap_fee)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.

    # Additional verification that swap limits are independent from set reverse swap limits call
    assert orig_swap_max == test.b.getSwapMax()
    assert orig_swap_min == test.b.getSwapMin()


def test_set_reverse_swap_limits_reverts(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    permutations = [
        (100, 5, 10),
        (5, 100, 10),
        (5, 10, 100),
        (10, 5, 100),
        (10, 100, 5)
    ]

    # Prove that boundary condition is still valid
    test.setReverseSwapLimits(swap_max=1, swap_min=1, swap_fee=1)

    # ===   WHEN / TEST SUBJECT  ==========================
    for max, min, fee in permutations:
        with brownie.reverts(revert_msg="fee <= min <= max violated"):
            test.setReverseSwapLimits(max, min, fee)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_set_swap_limits_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    orig_reverse_swap_max = test.b.getReverseSwapMax()
    orig_reverse_swap_min = test.b.getReverseSwapMin()
    orig_reverse_swap_fee = test.b.getReverseSwapFee()

    new_swap_max = test.b.getSwapMax() + 1
    new_swap_min = test.b.getSwapMin() + 1

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.setSwapLimits(new_swap_max, new_swap_min, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.setSwapLimits(new_swap_max, new_swap_min)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.

    # Additional verification that reverse swap limits are independent from set swap limits call
    assert orig_reverse_swap_max == test.b.getReverseSwapMax()
    assert orig_reverse_swap_min == test.b.getReverseSwapMin()
    assert orig_reverse_swap_fee == test.b.getReverseSwapFee()


def test_set_swap_limits_reverts(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    # Prove that boundary condition is still valid
    test.setSwapLimits(swap_max=1, swap_min=1)

    # ===   WHEN / TEST SUBJECT  ==========================
    with brownie.reverts(revert_msg="min <= max violated"):
       test.setSwapLimits(swap_max=10, swap_min=11)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_set_cap_basic(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    orig_cap = test.b.getCap()
    new_cap = orig_cap + 1

    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.setCap(new_cap, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.setCap(new_cap)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_all_contract_methods_for_adding_supply_revert_on_cap_violation(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test: BridgeTest = bridgeFactory()

    orig_supply = test.b.getSupply()
    room = test.vals.amount
    test.setCap(orig_supply + room)
    amount = room + 1

    # ===   WHEN / TEST SUBJECT  ==========================
    with brownie.reverts(revert_msg="Swap would exceed cap"):
        test.swap(user=test.users.users[0], amount=amount)

    with brownie.reverts(revert_msg="Deposit would exceed the cap"):
        test.deposit(amount)

    with brownie.reverts(revert_msg="Minting would exceed the cap"):
        test.mint(amount)

    # ===   THEN / VERIFICATION:  =========================
    # Verification is done by `brownie.reverts(...)` above


def test_set_reverse_aggregated_allowance_approver_cap(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    orig_apporver_cap = 10000
    test.bridge.reverseAggregatedAllowanceApproverCap = orig_apporver_cap
    test = bridgeFactory(test)

    new_expected_apporver_cap = orig_apporver_cap + 1

    # PFirst prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.setReverseAggregatedAllowanceApproverCap(new_expected_apporver_cap, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.setReverseAggregatedAllowanceApproverCap(new_expected_apporver_cap)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_set_reverse_aggregated_allowance_basic(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    orig_allowance = 1000
    new_expected_allowance = orig_allowance + 1
    approver_cap = new_expected_allowance

    test.bridge.reverseAggregatedAllowance = orig_allowance
    test.bridge.reverseAggregatedAllowanceApproverCap = approver_cap
    test = bridgeFactory(test)

    assert orig_allowance == test.b.getReverseAggregatedAllowance()

    # First prove negative:
    for u in test.users.canNOTSetReverseAggregatedAllowance:
        with brownie.reverts(revert_msg="Only admin or approver role"):
            test.setReverseAggregatedAllowance(new_expected_allowance, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    for u in test.users.canSetReverseAggregatedAllowance:
        test.setReverseAggregatedAllowance(new_expected_allowance, caller=u)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_set_reverse_aggregated_allowance_over_approver_cap(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    orig_allowance = 1000
    new_expected_allowance = orig_allowance + 1
    approver_cap = orig_allowance

    test.bridge.reverseAggregatedAllowance = orig_allowance
    test.bridge.reverseAggregatedAllowanceApproverCap = approver_cap
    test = bridgeFactory(test)

    assert orig_allowance == test.b.getReverseAggregatedAllowance()

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.setReverseAggregatedAllowance(new_expected_allowance, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.setReverseAggregatedAllowance(new_expected_allowance, caller=test.users.owner)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.


def test_set_reverse_aggregated_allowance_refund(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    amount = test.bridge.reverseSwapMin + test.bridge.reverseSwapFee

    test.bridge.reverseAggregatedAllowance = amount
    bridgeFactory(test)
    user = test.users.users[0]

    # adding supply
    tx0 = test.swap(user=user, amount=amount)
    tx1 = test.swap(user=user, amount=amount)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.refund(id=tx0.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    assert 0 == test.b.getReverseAggregatedAllowance()

    with brownie.reverts(revert_msg="Operation exceeds reverse aggregated allowance"):
        test.refund(id=tx1.events[str(EventType.Swap)]['id'], to_user=user, amount=1, waive_fee=False)


def test_set_reverse_aggregated_allowance_refund_in_full(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts)
    amount = test.bridge.reverseSwapMin + test.bridge.reverseSwapFee

    test.bridge.reverseAggregatedAllowance = amount
    bridgeFactory(test)
    user = test.users.users[0]

    # adding supply
    tx0 = test.swap(user=user, amount=amount)
    tx1 = test.swap(user=user, amount=amount)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.refund(id=tx0.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)
    assert 0 == test.b.getReverseAggregatedAllowance()

    with brownie.reverts(revert_msg="Operation exceeds reverse aggregated allowance"):
        test.refund(id=tx1.events[str(EventType.Swap)]['id'], to_user=user, amount=1, waive_fee=True)


def test_set_reverse_aggregated_allowance_reverse_swap(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = BridgeTest(accounts=accounts)
    amount = test.bridge.reverseSwapMin + test.bridge.reverseSwapFee

    test.bridge.reverseAggregatedAllowance = amount
    bridgeFactory(test)
    user = test.users.users[0]

    # adding supply
    tx0 = test.swap(user=user, amount=amount)
    tx1 = test.swap(user=user, amount=amount)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.reverseSwap(rid=0, to_user=user, amount=amount)
    assert 0 == test.b.getReverseAggregatedAllowance()

    with brownie.reverts(revert_msg="Operation exceeds reverse aggregated allowance"):
        test.reverseSwap(rid=1, to_user=user, amount=1)


def test_refund_reverts_for_swap_amount_bigger_than_swap_max_limit(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    test.standard_setup()

    user = test.users.users[0]
    amount = test.bridge.swapMin + test.bridge.reverseSwapFee

    test.setSwapLimits(
        swap_max=amount,
        swap_min=test.bridge.swapMin)

    # To prove that reverse swap limits are irrelevant for refunds
    test.setReverseSwapLimits(
        swap_max=0,
        swap_min=0,
        swap_fee=0)

    # Adding 4 swaps to bump up supply and to prepare for refund & reverseSwap calls
    txs = [test.swap(user=user, amount=amount) for _ in range(0,4)]

    # Prove negative (relative to test objective)
    test.refund(id=txs[0].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    test.refund(id=txs[1].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)

    test.setSwapLimits(
        swap_max=amount-1,
        swap_min=test.bridge.swapMin)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Amount exceeds swap max limit"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.refund(id=txs[2].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Amount exceeds swap max limit"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.refund(id=txs[3].events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=True)


def test_reverse_swap_reverts_for_swap_amount_bigger_than_reverse_swap_max_limit(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    test = bridgeFactory()

    test.standard_setup()

    user = test.users.users[0]
    amount = test.bridge.reverseSwapMin + test.bridge.reverseSwapFee

    # Ensuring that contract has enough funds for 2 reverse swaps
    deposit = 2 * amount
    test.t.approve(test.b.address, deposit, {'from': test.users.owner})
    test.deposit(deposit, caller=test.users.owner)

    test.setReverseSwapLimits(swap_max=amount,
                              swap_min=test.bridge.reverseSwapMin,
                              swap_fee=test.bridge.reverseSwapFee)

    # To prove that swap limits are irrelevant for reverse swaps
    test.setSwapLimits(swap_max=0,
                       swap_min=0)

    # Prove negative (relative to test objective)
    test.reverseSwap(rid=0, to_user=user, amount=amount)

    test.setReverseSwapLimits(swap_max=amount - 1,
                              swap_min=test.bridge.reverseSwapMin,
                              swap_fee=test.bridge.reverseSwapFee)

    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Amount exceeds swap max limit"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.reverseSwap(rid=0, to_user=user, amount=amount)


def test_fees_accrued(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    test = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.reverseSwapMax
    swap_fee = test.b.reverseSwapFee()
    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    test.t.transfer(test.b, excess_amount)
    test.swap(user=user, amount=amount)
    tx2 = test.swap(user=user, amount=amount)
    tx3 = test.swap(user=user, amount=amount)

    test.refund(id=tx2.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)
    test.refund(id=tx3.events[str(EventType.Swap)]['id'], to_user=user, amount=amount, waive_fee=False)

    # ===   WHEN / TEST SUBJECT  ==========================
    fees_accrued = test.b.getFeesAccrued()

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert fees_accrued == orig_fees_accrued + 2*swap_fee + excess_amount
    assert test.b.supply() == orig_supply + amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + amount + 2*swap_fee + excess_amount


def test_mint(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    mint_amount = 972
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.reverseSwapMax

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)

    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.mint(mint_amount, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.mint(mint_amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.getFeesAccrued() == orig_fees_accrued
    assert test.b.supply() == orig_supply + mint_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + mint_amount


def test_burn(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    amount = test.bridge.reverseSwapMax

    burn_amount = amount/2
    assert burn_amount > 0

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)

    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.burn(burn_amount, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.burn(burn_amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.getFeesAccrued() == orig_fees_accrued
    assert test.b.supply() == orig_supply - burn_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance - burn_amount


def test_deposit(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    deposit_amount = 972
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    from_user = test.users.owner
    amount = test.bridge.reverseSwapMax

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)

    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_from_user_balance = test.t.balanceOf(from_user)

    test.t.approve(test.b, deposit_amount, {'from': from_user})

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            tx = test.deposit(deposit_amount, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.deposit(deposit_amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.getFeesAccrued() == orig_fees_accrued
    assert test.b.supply() == orig_supply + deposit_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance + deposit_amount
    assert test.t.balanceOf(from_user) == orig_from_user_balance - deposit_amount


def test_withdraw(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.reverseSwapMax
    withdraw_amount = 972

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)

    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.withdraw(target_to_user, withdraw_amount, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    tx = test.withdraw(target_to_user, withdraw_amount)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.getFeesAccrued() == orig_fees_accrued
    assert test.b.supply() == orig_supply - withdraw_amount
    assert test.t.balanceOf(test.b) == orig_contract_balance - withdraw_amount
    assert test.t.balanceOf(target_to_user) == orig_target_to_user_balance + withdraw_amount


def test_withdraw_fees(bridgeFactory):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    test: BridgeTest = bridgeFactory()
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.reverseSwapMax

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)

    orig_supply = test.b.supply()
    orig_fees_accrued = test.b.getFeesAccrued()
    orig_contract_balance = test.t.balanceOf(test.b)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)

    # First prove negative:
    for u in test.users.notOwners:
        with brownie.reverts(revert_msg="Only admin role"):
            test.withdrawFees(target_to_user, caller=u)

    # ===   WHEN / TEST SUBJECT  ==========================
    test.withdrawFees(target_to_user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.b.getFeesAccrued() == 0
    assert test.b.supply() == orig_supply
    assert test.t.balanceOf(test.b) == orig_contract_balance - orig_fees_accrued
    assert test.t.balanceOf(target_to_user) == orig_target_to_user_balance + orig_fees_accrued


def test_delete_contract_reverts_when_protection_period_not_reached(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    delete_protection_period = 20
    test = BridgeTest(accounts=accounts)
    test.bridge.deleteProtectionPeriod = delete_protection_period
    bridgeFactory(test)
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.reverseSwapMax

    #test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)
    assert test.b.getEarliestDelete() > brownie.web3.eth.blockNumber

    # Negative test
    # ===   THEN / VERIFICATION:  =========================
    with brownie.reverts(revert_msg="Earliest delete not reached"):
        # ===   WHEN / TEST SUBJECT  ==========================
        test.deleteContract(target_to_user)


def test_delete_contract_reverts_due_to_access_rights(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    delete_protection_period = 20
    test = BridgeTest(accounts=accounts)
    test.bridge.deleteProtectionPeriod = delete_protection_period
    bridgeFactory(test)
    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.reverseSwapMax

    #test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)
    brownie.chain.mine(delete_protection_period)
    assert test.b.getEarliestDelete() <= brownie.web3.eth.blockNumber

    for u in test.users.notOwners:
        # ===   THEN / VERIFICATION:  =========================
        with brownie.reverts(revert_msg="Only admin role"):
            # ===   WHEN / TEST SUBJECT  ==========================
            test.deleteContract(target_to_user, caller=u)


def test_delete_contract_passes_when_protection_period_reached(bridgeFactory, accounts):
    # ===   GIVEN / PRECONDITIONS:  =======================
    excess_amount = 1234
    delete_protection_period = 20

    test = BridgeTest(accounts=accounts)
    test.bridge.deleteProtectionPeriod = delete_protection_period
    bridgeFactory(test)

    user = test.users.users[0]
    target_to_user = test.users.users[1]
    amount = test.bridge.reverseSwapMax
    contract_address = test.b.address

    test.standard_setup(user=user, amount=amount, excess_amount=excess_amount)
    assert test.b.getEarliestDelete() > brownie.web3.eth.blockNumber

    orig_contract_balance = test.t.balanceOf(contract_address)
    orig_target_to_user_balance = test.t.balanceOf(target_to_user)
    assert orig_contract_balance > 0

    brownie.chain.mine(delete_protection_period)
    assert test.b.getEarliestDelete() <= brownie.web3.eth.blockNumber

    # ===   WHEN / TEST SUBJECT  ==========================
    test.deleteContract(target_to_user)

    # ===   THEN / VERIFICATION:  =========================
    # All necessary verification is already implemented inside of the test subject method called above.
    # Just repeating basic check here again to make it explicit and visible:
    assert test.t.balanceOf(contract_address) == 0
    assert test.t.balanceOf(target_to_user) == orig_target_to_user_balance + orig_contract_balance
