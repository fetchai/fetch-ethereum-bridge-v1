// SPDX-License-Identifier:Apache-2.0
//------------------------------------------------------------------------------
//
//   Copyright 2021 Fetch.AI Limited
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.
//
//------------------------------------------------------------------------------

pragma solidity ^0.6.0 || ^0.7.0;

import "../openzeppelin/contracts/access/AccessControl.sol";
import "../openzeppelin/contracts/math/SafeMath.sol";
import "../interfaces/IERC20Token.sol";
import "../interfaces/IBridge.sol";


/**
 * @title Bi-directional bridge for transferring FET tokens between Ethereum and Fetch Mainnet-v2
 *
 * @notice This bridge allows to transfer [ERC20-FET] tokens from Ethereum Mainnet to [Native FET] tokens on Fetch
 *         Native Mainnet-v2 and **other way around** (= it is bi-directional).
 *         User will be *charged* swap fee defined in counterpart contract deployed on Fetch Native Mainnet-v2.
 *         In the case of a refund, user will be charged a swap fee configured in this contract.
 *
 * @dev There are three primary actions defining business logic of this contract:
 *       * `swap(...)`: initiates swap of tokens from Ethereum to Fetch Native Mainnet-v2, callable by anyone (= users)
 *       * `reverseSwap(...)`: finalises the swap of tokens in *opposite* direction = receives swap originally
 *                             initiated on Fetch Native Mainnet-v2, callable exclusively by `relayer` role
 *       * `refund(...)`: refunds swap originally initiated in this contract(by `swap(...)` call), callable exclusively
 *                        by `relayer` role
 *
 *      Swap Fees for `swap(...)` operations (direction from this contract to are handled by the counterpart contract on Fetch Native Mainnet-v2, **except** for refunds, for
 *      which user is charged swap fee defined by this contract (since relayer needs to send refund transaction back to
 *      this contract.
 *
 *      ! IMPORTANT !: Current design of this contract does *NOT* allow to distinguish between *swap fees accrued* and
 *      *excess funds* sent to the address of this contract via *direct* `ERC20.transfer(...)`.
 *      Implication is, that excess funds **are treated** as swap fees.
 *      The only way how to separate these two is to do it *off-chain*, by replaying events from this and FET ERC20
 *      contracts, and do the reconciliation.
 */
contract Bridge is IBridge, AccessControl {
    using SafeMath for uint256;

    /// @notice **********    CONSTANTS    ***********
    bytes32 public constant DELEGATE_ROLE = keccak256("DELEGATE_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");

    /// @notice *******    IMMUTABLE STATE    ********
    IERC20Token public immutable token;
    uint256 public immutable earliestDelete;
    /// @notice ********    MUTABLE STATE    *********
    uint256 public supply;
    uint64 public  nextSwapId;
    uint64 public  relayEon;
    mapping(uint64 => uint256) public refunds; // swapId -> original swap amount(= *includes* swapFee)
    uint256 public swapMax;
    uint256 public swapMin;
    uint256 public cap;
    uint256 public swapFee;
    uint256 public pausedSinceBlock;
    uint256 public reverseAggregate;
    uint256 public reverseAggregateCap;


    /* Only callable by owner */
    modifier onlyOwner() {
        require(_isOwner(), "Caller must be owner");
        _;
    }

    /* Only callable by owner or delegate */
    modifier onlyDelegate() {
        require(hasRole(DELEGATE_ROLE, msg.sender) || _isOwner(), "Caller must be owner or delegate");
        _;
    }

    modifier onlyRelayer() {
        require(hasRole(RELAYER_ROLE, msg.sender), "Caller must be relayer");
        _;
    }

    modifier canPause() {
        require(hasRole(RELAYER_ROLE, msg.sender) || _isOwner() || hasRole(DELEGATE_ROLE, msg.sender), "Only relayer, admin or delegate");
        _;
    }

    modifier verifyTxRelayEon(uint64 relayEon_) {
        require(relayEon == relayEon_, "Tx doesn't belong to current relayEon");
        _;
    }

    modifier verifyNotPaused() {
        require(pausedSinceBlock > block.number, "Contract has been paused");
        _;
    }

    modifier verifySwapAmount(uint256 amount) {
        // NOTE(pb): Commenting-out check against `swapFee` in order to spare gas for user's Tx, relying solely on check
        //  against `swapMin` only, which is ensured to be `>= swapFee` (by `_setLimits(...)` function).
        //require(amount > swapFee, "Amount must be higher than fee");
        require(amount >= swapMin, "Amount bellow lower limit");
        require(amount <= swapMax, "Amount exceeds upper limit");
        _;
    }

    modifier verifyRefundSwapId(uint64 id) {
        require(id < nextSwapId, "Invalid swap id");
        require(refunds[id] == 0, "Refund was already processed");
        _;
    }


    /*******************
    Contract start
    *******************/
    /**
     * @notice Contract constructor
     * @dev Input parameters offers full flexibility to configure the contract during deployment, with minimal need of
     *      further setup transactions necessary to open contract to the public.
     *
     * @param ERC20Address - address of FET ERC20 token contract
     * @param cap_ - limits contract `supply` value from top
     * @param swapMax_ - value representing UPPER limit which can be transferred (this value INCLUDES swapFee)
     * @param swapMin_ - value representing LOWER limit which can be transferred (this value INCLUDES swapFee)
     * @param swapFee_ - represents fee which user has to pay for swap execution,
     * @param pausedSinceBlock_ - block number since which the contract will be paused for all user-level actions
     * @param deleteProtectionPeriod_ - number of blocks(from contract deployment block) during which contract can
     *                                  NOT be deleted
     */
    constructor(
          address ERC20Address
        , uint256 cap_
        , uint256 swapMax_
        , uint256 swapMin_
        , uint256 swapFee_
        , uint256 pausedSinceBlock_
        , uint256 deleteProtectionPeriod_)
    {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        token = IERC20Token(ERC20Address);
        earliestDelete = block.number.add(deleteProtectionPeriod_);

        /// @dev Unnecessary initialisations, done implicitly by VM
        //supply = 0;
        //refundsFeesAccrued = 0;
        //nextSwapId = 0;

        // NOTE(pb): Initial value is by design set to MAX_LIMIT<uint64>, so that its NEXT increment(+1) will
        //           overflow to 0.
        relayEon = type(uint64).max;

        _setCap(cap_);
        _setLimits(swapMax_, swapMin_, swapFee_);
        _pauseSince(pausedSinceBlock_);
    }


    // **********************************************************
    // ***********    USER-LEVEL ACCESS METHODS    **********


    /**
      * @notice Initiates swap, which will be relayed to the other blockchain.
      *         Swap might fail, if `destinationAddress` value is invalid (see bellow), in which case the swap will be
      *         refunded back to user. Swap fee will be *WITHDRAWN* from `amount` in that case - please see details
      *         in desc. for `refund(...)` call.
      *
      * @dev Swap call will create unique identifier (swap id), which is, by design, sequentially growing by 1 per each
      *      new swap created, and so uniquely identifies each swap. This identifier is referred to as "reverse swap id"
      *      on the other blockchain.
      *      Callable by anyone.
      *
      * @param destinationAddress - address on **OTHER** blockchain where the swap effective amount will be transferred
      *                             in to.
      *                             User is **RESPONSIBLE** for providing the **CORRECT** and valid value.
      *                             The **CORRECT** means, in this context, that address is valid *AND* user really
      *                             intended this particular address value as destination = that address is NOT lets say
      *                             copy-paste mistake made by user. Reason being that when user provided valid address
      *                             value, but made mistake = address is of someone else (e.g. copy-paste mistake), then
      *                             there is **NOTHING** what can be done to recover funds back to user (= refund) once
      *                             the swap will be relayed to the other blockchain!
      *                             The **VALID** means that provided value successfully passes consistency checks of
      *                             valid address of **OTHER** blockchain. In the case when user provides invalid
      *                             address value, relayer will execute refund - please see desc. for `refund()` call
      *                             for more details.
      */
    function swap(
        uint256 amount, // This is original amount (INCLUDES fee)
        string calldata destinationAddress
        )
        external
        override
        verifyNotPaused
        verifySwapAmount(amount)
    {
        supply = supply.add(amount);
        require(cap >= supply, "Swap would exceed cap");
        token.transferFrom(msg.sender, address(this), amount);
        emit Swap(nextSwapId, msg.sender, destinationAddress, destinationAddress, amount);
        // NOTE(pb): No necessity to use SafeMath here:
        nextSwapId += 1;
    }


    /**
     * @notice Returns amount of excess FET ERC20 tokens which were sent to address of this contract via direct ERC20
     *         transfer (by calling ERC20.transfer(...)), without interacting with API of this contract, what can happen
     *         only by mistake.
     *
     * @return targetAddress : address to send tokens to
     */
    function getFeesAccrued() external view override returns(uint256) {
        // NOTE(pb): This subtraction shall NEVER fail:
        return token.balanceOf(address(this)).sub(supply, "Critical err: balance < supply");
    }

    function getDelegateRole() external view override returns(bytes32) {return DELEGATE_ROLE;}
    function getRelayerRole() external view override returns(bytes32) {return RELAYER_ROLE;}

    function getToken() external view override returns(address) {return address(token);}
    function getEarliestDelete() external view override returns(uint256) {return earliestDelete;}
    function getSupply() external view override returns(uint256) {return supply;}
    function getNextSwapId() external view override returns(uint64) {return nextSwapId;}
    function getRelayEon() external view override returns(uint64) {return relayEon;}
    function getRefund(uint64 swap_id) external view override returns(uint256) {return refunds[swap_id];}
    function getSwapMax() external view override returns(uint256) {return swapMax;}
    function getSwapMin() external view override returns(uint256) {return swapMin;}
    function getCap() external view override returns(uint256) {return cap;}
    function getSwapFee() external view override returns(uint256) {return swapFee;}
    function getPausedSinceBlock() external view override returns(uint256) {return pausedSinceBlock;}
    function getReverseAggregate() external view override returns(uint256) {return reverseAggregate;}
    function getReverseAggregateCap() external view override returns(uint256) {return reverseAggregateCap;}

    // **********************************************************
    // ***********    RELAYER-LEVEL ACCESS METHODS    ***********


    /**
      * @notice Starts the new relay eon.
      * @dev Relay eon concept is part of the design in order to ensure safe management of hand-over between two
      *      relayer services. It provides clean isolation of potentially still pending transactions from previous
      *      relayer svc and the current one.
      */
    function newRelayEon()
        external
        override
        onlyRelayer
    {
        // NOTE(pb): No need for safe math for this increment, since the MAX_LIMIT<uint64> is huge number (~10^19),
        //  there is no way that +1 incrementing from initial 0 value can possibly cause overflow in real world - that
        //  would require to send more than 10^19 transactions to reach that point.
        //  The only case, where this increment operation will lead to overflow, by-design, is the **VERY 1st**
        //  increment = very 1st call of this contract method, when the `relayEon` is by-design & intentionally
        //  initialised to MAX_LIMIT<uint64> value, so the resulting value of the `relayEon` after increment will be `0`
        relayEon += 1;
        emit NewRelayEon(relayEon);
    }


    /**
      * @notice Refunds swap previously created by `swap(...)` call to this contract. The `swapFee` is *NOT* refunded
      *         back to the user (this is by-design).
      *
      * @dev Callable exclusively by `relayer` role
      *
      * @param id - swap id to refund - must be swap id of swap originally created by `swap(...)` call to this contract,
      *             **NOT** *reverse* swap id!
      * @param to - address where the refund will be transferred in to(IDENTICAL to address used in associated `swap`
      *             call)
      * @param amount - original amount specified in associated `swap` call = it INCLUDES swap fee, which will be
      *                 withdrawn
      * @param relayEon_ - current relay eon, ensures safe management of relaying process
      */
    function refund(
        uint64 id,
        address to,
        uint256 amount,
        uint64 relayEon_
        )
        external
        override
        verifyTxRelayEon(relayEon_)
        onlyRelayer
        verifyRefundSwapId(id)
    {
        _updateReverseAggregate(amount);

        // NOTE(pb): Fail as early as possible - withdrawal from supply is most likely to fail comparing to rest of the
        //  operations bellow.
        supply = supply.sub(amount, "Amount exceeds contract supply");

        // NOTE(pb): Same calls are repeated in both branches of the if-else in order to minimise gas impact, comparing
        //  to implementation, where these calls would be present in the code just once, after if-else block.
        if (amount > swapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - swapFee;
            token.transfer(to, effectiveAmount);
            emit SwapRefund(id, to, effectiveAmount, swapFee);
        } else {
            // NOTE(pb): No transfer necessary in this case, since whole amount is taken as swap fee.
            emit SwapRefund(id, to, 0, amount);
        }

        // NOTE(pb): Here we need to record the original `amount` value (passed as input param) rather than
        //  `effectiveAmount` in order to make sure, that the value is **NOT** zero (so it is possible to detect
        //  existence of key-value record in the `refunds` mapping (this is done in the `verifyRefundSwapId(...)`
        //  modifier). This also means that relayer role shall call this `refund(...)` function only for `amount > 0`,
        //  otherways relayer will pay Tx fee for executing the transaction which will have *NO* effect.
        refunds[id] = amount;
    }


    /**
      * @notice Refunds swap previously created by `swap(...)` call to this contract, where `swapFee` *IS* refunded
      *         back to the user (= swap fee is waived = user will receive full `amount`).
      *         Purpose of this method is to enable full refund in the situations when it si not user's fault that
      *         swap needs to be refunded (e.g. when Fetch Native Mainnet-v2 will become unavailable for prolonged
      *         period of time, etc. ...).
      *
      * @dev Callable exclusively by `relayer` role
      *
      * @param id - swap id to refund - must be swap id of swap originally created by `swap(...)` call to this contract,
      *             **NOT** *reverse* swap id!
      * @param to - address where the refund will be transferred in to(IDENTICAL to address used in associated `swap`
      *             call)
      * @param amount - original amount specified in associated `swap` call = it INCLUDES swap fee, which will be
      *                 waived = user will receive whole `amount` value.
      *                 Pleas mind that `amount > 0`, otherways relayer will pay Tx fee for executing the transaction
      *                 which will have *NO* effect (= like this function `refundInFull(...)` would *not* have been
      *                 called at all!
      * @param relayEon_ - current relay eon, ensures safe management of relaying process
      */
    function refundInFull(
        uint64 id,
        address to,
        uint256 amount,
        uint64 relayEon_
        )
        external
        override
        verifyTxRelayEon(relayEon_)
        onlyRelayer
        verifyRefundSwapId(id)
    {
        _updateReverseAggregate(amount);

        // NOTE(pb): Fail as early as possible - withdrawal from supply is most likely to fail comparing to rest of the
        //  operations bellow.
        supply = supply.sub(amount, "Amount exceeds contract supply");

        token.transfer(to, amount);
        emit SwapRefund(id, to, amount, 0);

        // NOTE(pb): Here we need to record the original `amount` value (passed as input param) rather than
        //  `effectiveAmount` in order to make sure, that the value is **NOT** zero (so it is possible to detect
        //  existence of key-value record in the `refunds` mapping (this is done in the `verifyRefundSwapId(...)`
        //  modifier). This also means that relayer role shall call this function function only for `amount > 0`,
        //  otherways relayer will pay Tx fee for executing the transaction which will have *NO* effect.
        refunds[id] = amount;
    }


    /**
      * @notice Finalises swap initiated by counterpart contract on the other blockchain.
      *         This call sends swapped tokens to `to` address value user specified in original swap on the **OTHER**
      *         blockchain.
      *
      * @dev Callable exclusively by `relayer` role
      *
      * @param rid - reverse swap id - unique identifier of the swap initiated on the **OTHER** blockchain.
      *              This id is, by definition, sequentially growing number incremented by 1 for each new swap initiated
      *              the other blockchain. **However**, it is *NOT* ensured that *all* swaps from the other blockchain
      *              will be transferred to this (Ethereum) blockchain, since some of these swaps can be refunded back
      *              to users (on the other blockchain).
      * @param to - address where the refund will be transferred in to
      * @param from - source address from which user transferred tokens from on the other blockchain. Present primarily
      *               for purposes of quick querying of events on this blockchain.
      * @param originTxHash - transaction hash for swap initiated on the **OTHER** blockchain. Present in order to
      *                       create strong bond between this and other blockchain.
      * @param amount - original amount specified in associated swap initiated on the other blockchain.
      *                 Swap fee is *withdrawn* from the `amount` user specified in the swap on the other blockchain,
      *                 what means that user receives `amount - swapFee`, or *nothing* if `amount <= swapFee`.
      *                 Pleas mind that `amount > 0`, otherways relayer will pay Tx fee for executing the transaction
      *                 which will have *NO* effect (= like this function `refundInFull(...)` would *not* have been
      *                 called at all!
      * @param relayEon_ - current relay eon, ensures safe management of relaying process
      */
    function reverseSwap(
        uint64 rid, // Reverse swp id (from counterpart contract on other blockchain)
        address to,
        string calldata from,
        bytes32 originTxHash,
        uint256 amount, // This is original swap amount (= *includes* swapFee)
        uint64 relayEon_
        )
        external
        override
        verifyTxRelayEon(relayEon_)
        onlyRelayer
    {
         _updateReverseAggregate(amount);

        // NOTE(pb): Fail as early as possible - withdrawal from supply is most likely to fail comparing to rest of the
        //  operations bellow.
        supply = supply.sub(amount, "Amount exceeds contract supply");

        if (amount > swapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - swapFee;
            token.transfer(to, effectiveAmount);
            emit ReverseSwap(rid, to, from, originTxHash, effectiveAmount, swapFee);
        } else {
            // NOTE(pb): No transfer, no contract supply change since whole amount is taken as swap fee.
            emit ReverseSwap(rid, to, from, originTxHash, 0, amount);
        }
    }


    // **********************************************************
    // ****   RELAYER/DELEGATE/ADMIN-LEVEL ACCESS METHODS   *****


    /**
     * @notice Pauses all NON-administrative interaction with the contract since the specified block number
     * @param blockNumber block number since which non-admin interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pauseSince(uint256 blockNumber)
        external
        override
        canPause
    {
        _pauseSince(blockNumber);
    }


    // **********************************************************
    // ************    ADMIN-LEVEL ACCESS METHODS   *************


    /**
     * @notice Mints provided amount of FET tokens.
     *         This is to reflect changes in minted Native FET token supply on the Fetch Native Mainnet-v2 blockchain.
     * @param amount - number of FET tokens to mint.
     */
    function mint(uint256 amount)
        external
        override
        onlyOwner
    {
        // NOTE(pb): The `supply` shall be adjusted by minted amount.
        supply = supply.add(amount);
        require(cap >= supply, "Minting would exceed the cap");
        token.mint(address(this), amount);
    }

    /**
     * @notice Burns provided amount of FET tokens.
     *         This is to reflect changes in minted Native FET token supply on the Fetch Native Mainnet-v2 blockchain.
     * @param amount - number of FET tokens to burn.
     */
    function burn(uint256 amount)
        external
        override
        onlyOwner
    {
        // NOTE(pb): The `supply` shall be adjusted by burned amount.
        supply = supply.sub(amount, "Amount exceeds contract supply");
        token.burn(amount);
    }


    /**
     * @notice Sets cap (max) value of `supply` this contract can hold = the value of tokens transferred to the other
     *         blockchain.
     *         This cap affects(limits) all operations which *increase* contract's `supply` value = `swap(...)` and
     *         `mint(...)`.
     * @param value - new cap value.
     */
    function setCap(uint256 value)
        external
        override
        onlyOwner
    {
        _setCap(value);
    }


    /**
     * @notice Sets cap (max) value of `reverseAggregate`
     *         This cap affects(limits) operations which *decrease* contract's `supply` value via **RELAYER**
     *          authored (= `reverseSwap(...)` and `refund(...)`). It does **NOT** limit `withdraw` & `burn` operations.
     * @param value - new cap value.
     */
    function setReverseAggregateCap(uint256 value)
        external
        override
        onlyOwner
    {
        _setReverseAggregateCap(value);
    }


    /**
     * @notice Sets limits for swap amount
     *         FUnction will revert if following consitency check fails: `swapfee_ <= swapMin_ <= swapMax_`
     * @param swapMax_ : >= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param swapMin_ : <= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param swapFee_ : defines swap fee for **INCOMING** swap (= `reverseSwap(...)` call), and `refund(...)`
     */
    function setLimits(
        uint256 swapMax_,
        uint256 swapMin_,
        uint256 swapFee_
        )
        external
        override
        onlyOwner
    {
        _setLimits(swapMax_, swapMin_, swapFee_);
    }


    /**
     * @notice Withdraws amount from contract's supply, which is supposed to be done exclusively for relocating funds to
     *       another Bridge system, and **NO** other purpose.
     * @param targetAddress : address to send tokens to
     * @param amount : amount of tokens to withdraw
     */
    function withdraw(
        address targetAddress,
        uint256 amount
        )
        external
        override
        onlyOwner
    {
        supply = supply.sub(amount, "Amount exceeds contract supply");
        token.transfer(targetAddress, amount);
        emit Withdraw(targetAddress, amount);
    }


    /**
     * @dev Deposits funds back in to the contract supply.
     *      Dedicated to increase contract's supply, usually(but not necessarily) after previous withdrawal from supply.
     *      NOTE: This call needs preexisting ERC20 allowance >= `amount` for address of this Bridge contract as
     *            recipient/beneficiary and Tx sender address as sender.
     *            This means that address passed in as the Tx sender, must have already crated allowance by calling the
     *            `ERC20.approve(from, ADDR_OF_BRIDGE_CONTRACT, amount)` *before* calling this(`deposit(...)`) call.
     * @param amount : deposit amount
     */
    function deposit(uint256 amount)
        external
        override
        onlyOwner
    {
        supply = supply.add(amount);
        require(cap >= supply, "Deposit would exceed the cap");
        token.transferFrom(msg.sender, address(this), amount);
        emit Deposit(msg.sender, amount);
    }


    /**
     * @notice Withdraw fees accrued so far.
     *         !IMPORTANT!: Current design of this contract does *NOT* allow to distinguish between *swap fees accrued*
     *                      and *excess funds* sent to the contract's address via *direct* `ERC20.transfer(...)`.
     *                      Implication is that excess funds **are treated** as swap fees.
     *                      The only way how to separate these two is off-chain, by replaying events from this and
     *                      Fet ERC20 contracts and do the reconciliation.
     *
     * @param targetAddress : address to send tokens to.
     */
    function withdrawFees(address targetAddress)
        external
        override
        onlyOwner
    {
        uint256 fees = this.getFeesAccrued();
        require(fees > 0, "No fees to withdraw");
        token.transfer(targetAddress, fees);
        emit FeesWithdrawal(targetAddress, fees);
    }


    /**
     * @notice Delete the contract, transfers the remaining token and ether balance to the specified
     *         payoutAddress
     * @param targetAddress address to transfer the balances to. Ensure that this is able to handle ERC20 tokens
     * @dev owner only + only on or after `earliestDelete` block
     */
    function deleteContract(address payable targetAddress)
        external
        override
        onlyOwner
    {
        require(earliestDelete >= block.number, "Earliest delete not reached");
        require(targetAddress != address(this), "pay addr == this contract addr");
        uint256 contractBalance = token.balanceOf(address(this));
        token.transfer(targetAddress, contractBalance);
        emit DeleteContract(targetAddress, contractBalance);
        selfdestruct(targetAddress);
    }


    // **********************************************************
    // ******************    INTERNAL METHODS   *****************


    function _isOwner() internal view returns(bool) {
        return hasRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }


    /**
     * @notice Pauses all NON-administrative interaction with the contract since the specidfed block number
     * @param blockNumber - block number since which non-admin interaction will be paused (for all
     *                      block.number >= blockNumber)
     */
    function _pauseSince(uint256 blockNumber) internal
    {
        pausedSinceBlock = blockNumber < block.number ? block.number : blockNumber;
        emit Pause(pausedSinceBlock);
    }


    function _setLimits(
        uint256 swapMax_,
        uint256 swapMin_,
        uint256 swapFee_
        )
        internal
    {
        require((swapFee_ <= swapMin_) && (swapMin_ <= swapMax_), "fee<=lower<=upper violated");

        swapMax = swapMax_;
        swapMin = swapMin_;
        swapFee = swapFee_;

        emit LimitsUpdate(swapMax, swapMin, swapFee);
    }


    function _setCap(uint256 cap_) internal
    {
        cap = cap_;
        emit CapUpdate(cap);
    }


    function _setReverseAggregateCap(uint256 value) internal
    {
        reverseAggregateCap = value;
        emit ReverseAggregateCapUpdate(reverseAggregateCap);
    }


    function _updateReverseAggregate(uint256 amount) internal {
        reverseAggregate += amount;
        require(reverseAggregate <= reverseAggregateCap, "Operation exceeds reverse aggregated cap");
    }
}
