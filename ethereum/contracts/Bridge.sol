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
    bytes32 public constant APPROVER_ROLE = keccak256("APPROVER_ROLE");
    bytes32 public constant MONITOR_ROLE = keccak256("MONITOR_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");

    /// @notice *******    IMMUTABLE STATE    ********
    IERC20Token public immutable token;
    uint256 public immutable earliestDelete;
    /// @notice ********    MUTABLE STATE    *********
    uint256 public supply;
    uint64 public  nextSwapId;
    uint64 public  relayEon;
    mapping(uint64 => uint256) public refunds; // swapId -> original swap amount(= *includes* reverseSwapFee)
    uint256 public swapMax;
    uint256 public swapMin;
    uint256 public reverseSwapMax;
    uint256 public reverseSwapMin;
    uint256 public reverseSwapFee;
    uint256 public pausedSinceBlockPublicApi;
    uint256 public pausedSinceBlockRelayerApi;
    uint256 public reverseAggregatedAllowance;
    uint256 public reverseAggregatedAllowanceApproverCap;
    uint256 public cap;


    /* Only callable by owner */
    modifier onlyOwner() {
        require(_isOwner(), "Only admin role");
        _;
    }

    modifier onlyRelayer() {
        require(hasRole(RELAYER_ROLE, msg.sender), "Only relayer role");
        _;
    }

    modifier verifyTxRelayEon(uint64 relayEon_) {
        require(relayEon == relayEon_, "Tx doesn't belong to current relayEon");
        _;
    }

    modifier canPause(uint256 pauseSinceBlockNumber) {
        if (pauseSinceBlockNumber > block.number) // Checking UN-pausing (the most critical operation)
        {
            require(_isOwner(), "Only admin role");
        }
        else
        {
            require(hasRole(MONITOR_ROLE, msg.sender) || _isOwner(), "Only admin or monitor role");
        }
        _;
    }

    modifier canSetReverseAggregatedAllowance(uint256 allowance) {
        if (allowance > reverseAggregatedAllowanceApproverCap) // Check for going over the approver cap (the most critical operation)
        {
            require(_isOwner(), "Only admin role");
        }
        else
        {
            require(hasRole(APPROVER_ROLE, msg.sender) || _isOwner(), "Only admin or approver role");
        }
        _;
    }

    modifier verifyPublicAPINotPaused() {
        require(pausedSinceBlockPublicApi > block.number, "Contract has been paused");
        _;
    }

    modifier verifyRelayerApiNotPaused() {
        require(pausedSinceBlockRelayerApi > block.number, "Contract has been paused");
        _;
    }

    modifier verifySwapAmount(uint256 amount) {
        require(amount >= swapMin, "Amount below swap min limit");
        require(amount <= swapMax, "Amount exceeds swap max limit");
        _;
    }

    modifier verifyRefundAmount(uint256 amount) {
        require(amount <= swapMax, "Amount exceeds swap max limit");
        _;
    }

    modifier verifyReverseSwapAmount(uint256 amount) {
        require(amount <= reverseSwapMax, "Amount exceeds swap max limit");
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
     * @param reverseAggregatedAllowance_ - allowance value which limits how much can refund & reverseSwap transfer
     *                                      in aggregated form
     * @param reverseAggregatedAllowanceApproverCap_ - limits allowance value up to which can APPROVER_ROLE set
     *                                                 the allowance
     * @param reverseSwapMax_ - value representing UPPER limit which can be transferred (this value INCLUDES reverseSwapFee)
     * @param reverseSwapMin_ - value representing LOWER limit which can be transferred (this value INCLUDES reverseSwapFee)
     * @param reverseSwapFee_ - represents fee which user has to pay for swap execution,
     * @param pausedSinceBlockPublicApi_ - block number since which the Public API of the contract will be paused
     * @param pausedSinceBlockRelayerApi_ - block number since which the Relayer API of the contract will be paused
     * @param deleteProtectionPeriod_ - number of blocks(from contract deployment block) during which contract can
     *                                  NOT be deleted
     */
    constructor(
          address ERC20Address
        , uint256 cap_
        , uint256 reverseAggregatedAllowance_
        , uint256 reverseAggregatedAllowanceApproverCap_
        , uint256 swapMax_
        , uint256 swapMin_
        , uint256 reverseSwapMax_
        , uint256 reverseSwapMin_
        , uint256 reverseSwapFee_
        , uint256 pausedSinceBlockPublicApi_
        , uint256 pausedSinceBlockRelayerApi_
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
        _setReverseAggregatedAllowance(reverseAggregatedAllowance_);
        _setReverseAggregatedAllowanceApproverCap(reverseAggregatedAllowanceApproverCap_);
        _setSwapLimits(swapMax_, swapMin_);
        _setReverseSwapLimits(reverseSwapMax_, reverseSwapMin_, reverseSwapFee_);
        _pausePublicApiSince(pausedSinceBlockPublicApi_);
        _pauseRelayerApiSince(pausedSinceBlockRelayerApi_);
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
        verifyPublicAPINotPaused
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

    function getApproverRole() external view override returns(bytes32) {return APPROVER_ROLE;}
    function getMonitorRole() external view override returns(bytes32) {return MONITOR_ROLE;}
    function getRelayerRole() external view override returns(bytes32) {return RELAYER_ROLE;}

    function getToken() external view override returns(address) {return address(token);}
    function getEarliestDelete() external view override returns(uint256) {return earliestDelete;}
    function getSupply() external view override returns(uint256) {return supply;}
    function getNextSwapId() external view override returns(uint64) {return nextSwapId;}
    function getRelayEon() external view override returns(uint64) {return relayEon;}
    function getRefund(uint64 swap_id) external view override returns(uint256) {return refunds[swap_id];}
    function getSwapMax() external view override returns(uint256) {return swapMax;}
    function getSwapMin() external view override returns(uint256) {return swapMin;}
    function getReverseSwapMax() external view override returns(uint256) {return reverseSwapMax;}
    function getReverseSwapMin() external view override returns(uint256) {return reverseSwapMin;}
    function getReverseSwapFee() external view override returns(uint256) {return reverseSwapFee;}
    function getPausedSinceBlockPublicApi() external view override returns(uint256) {return pausedSinceBlockPublicApi;}
    function getPausedSinceBlockRelayerApi() external view override returns(uint256) {return pausedSinceBlockRelayerApi;}
    function getReverseAggregatedAllowance() external view override returns(uint256) {return reverseAggregatedAllowance;}
    function getReverseAggregatedAllowanceApproverCap() external view override returns(uint256) {return reverseAggregatedAllowanceApproverCap;}
    function getCap() external view override returns(uint256) {return cap;}


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
        verifyRelayerApiNotPaused
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
      * @notice Refunds swap previously created by `swap(...)` call to this contract. The `reverseSwapFee` is *NOT* refunded
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
        verifyRelayerApiNotPaused
        verifyTxRelayEon(relayEon_)
        verifyRefundAmount(amount)
        onlyRelayer
        verifyRefundSwapId(id)
    {
        // NOTE(pb): Fail as early as possible - withdrawal from aggregated allowance is most likely to fail comparing
        //  to rest of the operations bellow.
        _updateReverseAggregatedAllowance(amount);

        supply = supply.sub(amount, "Amount exceeds contract supply");

        // NOTE(pb): Same calls are repeated in both branches of the if-else in order to minimise gas impact, comparing
        //  to implementation, where these calls would be present in the code just once, after if-else block.
        if (amount > reverseSwapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - reverseSwapFee;
            token.transfer(to, effectiveAmount);
            emit SwapRefund(id, to, effectiveAmount, reverseSwapFee);
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
      * @notice Refunds swap previously created by `swap(...)` call to this contract, where `reverseSwapFee` *IS* refunded
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
        verifyRelayerApiNotPaused
        verifyTxRelayEon(relayEon_)
        verifyRefundAmount(amount)
        onlyRelayer
        verifyRefundSwapId(id)
    {
        // NOTE(pb): Fail as early as possible - withdrawal from aggregated allowance is most likely to fail comparing
        //  to rest of the operations bellow.
        _updateReverseAggregatedAllowance(amount);

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
      *                 what means that user receives `amount - reverseSwapFee`, or *nothing* if `amount <= reverseSwapFee`.
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
        uint256 amount, // This is original swap amount (= *includes* reverseSwapFee)
        uint64 relayEon_
        )
        external
        override
        verifyRelayerApiNotPaused
        verifyTxRelayEon(relayEon_)
        verifyReverseSwapAmount(amount)
        onlyRelayer
    {
        // NOTE(pb): Fail as early as possible - withdrawal from aggregated allowance is most likely to fail comparing
        //  to rest of the operations bellow.
        _updateReverseAggregatedAllowance(amount);

        supply = supply.sub(amount, "Amount exceeds contract supply");

        if (amount > reverseSwapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - reverseSwapFee;
            token.transfer(to, effectiveAmount);
            emit ReverseSwap(rid, to, from, originTxHash, effectiveAmount, reverseSwapFee);
        } else {
            // NOTE(pb): No transfer, no contract supply change since whole amount is taken as swap fee.
            emit ReverseSwap(rid, to, from, originTxHash, 0, amount);
        }
    }


    // **********************************************************
    // ****   MONITOR/ADMIN-LEVEL ACCESS METHODS   *****


    /**
     * @notice Pauses Public API since the specified block number
     * @param blockNumber block number since which public interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pausePublicApiSince(uint256 blockNumber)
        external
        override
        canPause(blockNumber)
    {
        _pausePublicApiSince(blockNumber);
    }


    /**
     * @notice Pauses Relayer API since the specified block number
     * @param blockNumber block number since which Relayer API interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pauseRelayerApiSince(uint256 blockNumber)
        external
        override
        canPause(blockNumber)
    {
        _pauseRelayerApiSince(blockNumber);
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
     *         This cap affects(limits) public operations which *increase* contract's `supply` value (= `swap(...)`),
     *         however admin level operations are **NOT** limited (= `deposit(...)` & `mint(...)`).
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
     * @notice Sets value of `reverseAggregatedAllowance` state variable.
     *         This affects(limits) operations which *decrease* contract's `supply` value via **RELAYER** authored
     *         operations (= `reverseSwap(...)` and `refund(...)`). It does **NOT** affect **ADMINISTRATION** authored
     *         supply decrease operations (= `withdraw(...)` & `burn(...)`).
     * @param value - new allowance value (absolute)
     */
    function setReverseAggregatedAllowance(uint256 value)
        external
        override
        canSetReverseAggregatedAllowance(value)
    {
        _setReverseAggregatedAllowance(value);
    }


    /**
     * @notice Sets value of `reverseAggregatedAllowanceApproverCap` state variable.
     *         This limits APPROVER_ROLE from top - value up to which can approver rise the allowance.
     * @param value - new cap value (absolute)
     */
    function setReverseAggregatedAllowanceApproverCap(uint256 value)
        external
        override
        onlyOwner
    {
        _setReverseAggregatedAllowanceApproverCap(value);
    }


    /**
     * @notice Sets limits for amount value provided in `swap(...)` call
     *         Method call will revert if following consistency check fails: `swapMin_ <= swapMax_`
     * @param swapMax_ : >= swap amount of **OUTGOING** `swap(...)` call
     * @param swapMin_ : <= swap amount of **OUTGOING** `swap(...)` call
     */
    function setSwapLimits(
        uint256 swapMax_,
        uint256 swapMin_
        )
        external
        override
        onlyOwner
    {
        _setSwapLimits(swapMax_, swapMin_);
    }


    /**
     * @notice Sets limits for amount provided in all reverse swap operations
     *         FUnction will revert if following consistency check fails: `reverseSwapFee_ <= reverseSwapMin_ <= reverseSwapMax_`
     * @param reverseSwapMax_ : >= reverse swap amount, applies for **INCOMING** reverse swap (= `reverseSwap(...)` call)
     * @param reverseSwapMin_ : <= reverse swap amount, applies for **INCOMING** reverse swap (= `reverseSwap(...)` call)
     * @param reverseSwapFee_ : defines swap fee for **INCOMING** reverse swap (= `reverseSwap(...)` and `refund...(...) calls)`
     */
    function setReverseSwapLimits(
        uint256 reverseSwapMax_,
        uint256 reverseSwapMin_,
        uint256 reverseSwapFee_
        )
        external
        override
        onlyOwner
    {
        _setReverseSwapLimits(reverseSwapMax_, reverseSwapMin_, reverseSwapFee_);
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
        require(earliestDelete <= block.number, "Earliest delete not reached");
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
     * @notice Pauses Public API since the specified block number
     * @param blockNumber - block number since which interaction with Public API will be paused (for all
     *                      block.number >= blockNumber)
     */
    function _pausePublicApiSince(uint256 blockNumber) internal
    {
        pausedSinceBlockPublicApi = blockNumber < block.number ? block.number : blockNumber;
        emit PausePublicApi(pausedSinceBlockPublicApi);
    }


    /**
     * @notice Pauses Relayer API since the specified block number
     * @param blockNumber - block number since which interaction with Relayer API will be paused (for all
     *                      block.number >= blockNumber)
     */
    function _pauseRelayerApiSince(uint256 blockNumber) internal
    {
        pausedSinceBlockRelayerApi = blockNumber < block.number ? block.number : blockNumber;
        emit PauseRelayerApi(pausedSinceBlockRelayerApi);
    }


    function _setSwapLimits(
        uint256 swapMax_,
        uint256 swapMin_
        )
        internal
    {
        require(swapMin_ <= swapMax_, "min <= max violated");

        swapMax = swapMax_;
        swapMin = swapMin_;

        emit SwapLimitsUpdate(swapMax, swapMin);
    }


    function _setReverseSwapLimits(
        uint256 reverseSwapMax_,
        uint256 reverseSwapMin_,
        uint256 reverseSwapFee_
        )
        internal
    {
        require((reverseSwapFee_ <= reverseSwapMin_) && (reverseSwapMin_ <= reverseSwapMax_), "fee <= min <= max violated");

        reverseSwapMax = reverseSwapMax_;
        reverseSwapMin = reverseSwapMin_;
        reverseSwapFee = reverseSwapFee_;

        emit ReverseSwapLimitsUpdate(reverseSwapMax, reverseSwapMin, reverseSwapFee);
    }


    function _setCap(uint256 cap_) internal
    {
        cap = cap_;
        emit CapUpdate(cap);
    }


    function _setReverseAggregatedAllowance(uint256 allowance) internal
    {
        reverseAggregatedAllowance = allowance;
        emit ReverseAggregatedAllowanceUpdate(reverseAggregatedAllowance);
    }


    function _setReverseAggregatedAllowanceApproverCap(uint256 value) internal
    {
        reverseAggregatedAllowanceApproverCap = value;
        emit ReverseAggregatedAllowanceApproverCapUpdate(reverseAggregatedAllowanceApproverCap);
    }


    function _updateReverseAggregatedAllowance(uint256 amount) internal {
        reverseAggregatedAllowance = reverseAggregatedAllowance.sub(amount, "Operation exceeds reverse aggregated allowance");
    }
}
