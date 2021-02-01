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
import "./IERC20Token.sol";


/**
 * @title Bi-directional bridge for transferring FET tokens between Ethereum and Fetch Mainnet-v2
 * @notice This bridge allows to transfer [ERC20-FET] tokens from Ethereum Mainnet to [Native FET] tokens on Fetch
 *         Native Mainnet-v2 and **other way around** (= it is bi-directional).
 *         User will be *charged* swap fee defined in counterpart contract deployed on Fetch Native Mainnet-v2.
 *         In the case of a refund, user will be charged a swap fee configured in this contract.
 * @dev There are three primary actions defining business logic of this contract:
 *       * `swap(...)`: initiates swap of tokens from Ethereum to Fetch Native Mainnet-v2, callable by anyone (= users)
 *       * `reverseSwap(...)`: finalises the swap of tokens in *opposite* direction = receives swap originally
 *                             initiated on Fetch Native Mainnet-v2, callable exclusively by `relayer` role
 *       * `refund(...)`: refunds swap originally initiated in this contract(by `swap(...)` call), callable exclusively
 *                        by `relayer` role
 *      Swap Fees are handled by the counterpart contract on Fetch Native Mainnet-v2, **except** for refunds, for
 *      which user is changed swap fee defined by this contract (since relayer needs to send refund transaction back to
 *      this contract.
 */
contract Bridge is AccessControl {
    using SafeMath for uint256;

    // *******    USER-LEVEL EVENTS    ********
    event Swap(uint256 indexed id, string indexed to, uint256 amount);
    // *******    DELEGATE-LEVEL EVENTS    ********
    event SwapRefund(uint256 indexed id, address indexed to, uint256 refundedAmount, uint256 fee);
    event ReverseSwap(uint256 indexed rid, address indexed to, string indexed from, bytes32 originTxHash, uint256 amount, uint256 fee);
    event Pause(uint256 sinceBlock);
    // *******    ADMIN-LEVEL EVENTS    ********
    event LimitsUpdate(uint256 upperSwqpLimit, uint256 lowerSwapLimit, uint256 swapFee);
    event CapUpdate(uint256 amount);
    event NewRelayEon(uint64 eon);
    event Withdraw(address indexed targetAddress, uint256 amount);
    event Deposit(address indexed fromAddress, uint256 amount);
    event RefundsFeesWithdrawal(address indexed targetAddress, uint256 amount);
    event ExcessFundsWithdrawal(address indexed targetAddress, uint256 tokenAmount, uint256 ethAmount);
    event DeleteContract(address payoutAddress);
    // NOTE(pb): It is NOT necessary to have dedicated events here for Mint & Burn operations, since ERC20 contract
    //  already emits the `Transfer(from, to, amount)` events, with `from`, resp. `to`, address parameter value set to
    //  ZERO_ADDRESS (= address(0) = 0x00...00) for `mint`, resp `burn`, calls to ERC20 contract. That way we can
    //  identify events for mint, resp. burn, calls by filtering ERC20 Transfer events with `from == ZERO_ADDR  &&
    //  to == Bridge.address` for MINT operation, resp `from == Bridge.address` and `to == ZERO_ADDR` for BURN operation.
    //event Mint(uint256 amount);
    //event Burn(uint256 amount);

    /// @notice **********    CONSTANTS    ***********
    bytes32 public constant DELEGATE_ROLE = keccak256("DELEGATE_ROLE");
    bytes32 public constant RELAYER_ROLE = keccak256("RELAYER_ROLE");

    /// @notice *******    IMMUTABLE STATE    ********
    IERC20Token public immutable token;
    uint256 public immutable earliestDelete;
    /// @notice ********    MUTABLE STATE    *********
    uint256 public supply;
    uint256 public refundsFeesAccrued;
    uint64 public  nextSwapId;
    uint64 public  relayEon;
    mapping(uint64 => uint256) public refunds; // swapId -> original swap amount(= *includes* swapFee)
    uint256 public upperSwapLimit;
    uint256 public lowerSwapLimit;
    uint256 public cap;
    uint256 public swapFee;
    uint256 public pausedSinceBlock;



    /* Only callable by owner */
    modifier onlyOwner() {
        require(_isOwner(), "Caller is not an owner");
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
        require(pausedSinceBlock > _getBlockNumber(), "Contract has been paused");
        _;
    }

    modifier verifySwapAmount(uint256 amount) {
        // NOTE(pb): Commenting-out check against `swapFee` in order to spare gas for user's Tx, relying solely on check
        //  against `lowerSwapLimit` only, which is ensured to be `>= swapFee` (by `_setLimits(...)` function).
        //require(amount > swapFee, "Amount must be higher than fee");
        require(amount >= lowerSwapLimit, "Amount bellow lower limit");
        require(amount <= upperSwapLimit, "Amount exceeds upper limit");
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
     * @param upperSwapLimit_ - value representing UPPER limit which can be transferred (this value INCLUDES swapFee)
     * @param lowerSwapLimit_ - value representing LOWER limit which can be transferred (this value INCLUDES swapFee)
     * @param swapFee_ - represents fee which user has to pay for swap execution,
     * @param pausedSinceBlock_ - block number since which the contract will be paused for all user-level actions
     * @param deleteProtectionPeriod_ - number of blocks(from contract deployment block) during which contract can
     *                                  NOT be deleted
     */
    constructor(
          address ERC20Address
        , uint256 cap_
        , uint256 upperSwapLimit_
        , uint256 lowerSwapLimit_
        , uint256 swapFee_
        , uint256 pausedSinceBlock_
        , uint256 deleteProtectionPeriod_)
    {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        token = IERC20Token(ERC20Address);
        earliestDelete = _getBlockNumber().add(deleteProtectionPeriod_);

        /// @dev Unnecessary initialisations, done implicitly by VM
        //supply = 0;
        //refundsFeesAccrued = 0;
        //nextSwapId = 0;

        // NOTE(pb): Initial value is by design set to MAX_LIMIT<uint64>, so that its NEXT increment(+1) will
        //           overflow to 0.
        relayEon = type(uint64).max;

        _setCap(cap_);
        _setLimits(upperSwapLimit_, lowerSwapLimit_, swapFee_);
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
      *                             The **CORRECT** means, in this context, that address is valid AND user really
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
        public
        verifyNotPaused
        verifySwapAmount(amount)
    {
        supply = supply.add(amount);
        require(cap >= supply, "Swap would exceed cap");
        token.transferFrom(msg.sender, address(this), amount);
        emit Swap(nextSwapId, destinationAddress, amount);
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
    function getExcessFunds() public view returns(uint256) {
        return _excessFunds();
    }


    // **********************************************************
    // ***********    RELAYER-LEVEL ACCESS METHODS    ***********


    /**
      * @notice Starts the new relay eon.
      * @dev Relay eon concept is part of the design in order to ensure safe management of hand-over between two
      *      relayer services. It provides clean isolation of potentially still pending transactions from previous
      *      relayer svc and the current one.
      */
    function newRelayEon()
        public
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
        public
        verifyTxRelayEon(relayEon_)
        onlyRelayer
        verifyRefundSwapId(id)
    {
        // NOTE(pb): Same calls are repeated in both branches of the if-else in order to minimise gas impact, comparing
        //  to implementation, where these calls would be present in the code just once, after if-else block.
        if (amount > swapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - swapFee;
            token.transfer(to, effectiveAmount);

            refundsFeesAccrued = refundsFeesAccrued.add(swapFee);
            emit SwapRefund(id, to, effectiveAmount, swapFee);
        } else {
            // NOTE(pb): No transfer necessary in this case, since whole amount is taken as swap fee.
            refundsFeesAccrued = refundsFeesAccrued.add(amount);
            emit SwapRefund(id, to, 0, amount);
        }

        // NOTE(pb): Whole `amount` **MUST** be withdrawn from `supply` in order to preserve the exact balance with
        //  `supply` of counterpart contract, since original swap amount is **NO** more part of supply **after** it
        //  has been refunded (= it has **NOT** been, and **NEVER** will be, transferred to counterpart contract).
        supply = supply.sub(amount);

        // NOTE(pb): Here we need to record the original `amount` value (passed as input param) rather than
        //  `effectiveAmount` in order to make sure, that the value is **NOT** zero (so it is possible to detect
        //  existence of key-value record in the `refunds` mapping (this is done in the `verifyRefundSwapId(...)`
        //  modifier). This also means that relayer role shall call this `refund(...)` function only for `amount > 0`,
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
        public
        verifyTxRelayEon(relayEon_)
        onlyRelayer
    {
        if (amount > swapFee) {
            // NOTE(pb): No need to use safe math here, the overflow is prevented by `if` condition above.
            uint256 effectiveAmount = amount - swapFee;

            token.transfer(to, effectiveAmount);
            // NOTE(pb): In theory, SafeMath should not be necessary for the following sub., left in for peace in mind:
            supply = supply.sub(effectiveAmount);
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
     *        _getBlockNumber() >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < _getBlockNumber()`, then contract will be paused immediately = from `_getBlockNumber()`.
     */
    function pauseSince(uint256 blockNumber)
        public
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
        public
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
        public
        onlyOwner
    {
        // NOTE(pb): The `supply` shall be adjusted by burned amount.
        supply = supply.sub(amount);
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
        public
        onlyOwner
    {
        _setCap(value);
    }


    /**
     * @notice Sets limits for swap amount
     *
     * @param upperSwapLimit_ : >= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param lowerSwapLimit_ : <= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param swapFee_ : defines swap fee for **INCOMING** swap (= `reverseSwap(...)` call), and `refund(...)`
     */
    function setLimits(
        uint256 upperSwapLimit_,
        uint256 lowerSwapLimit_,
        uint256 swapFee_
        )
        public
        onlyOwner
    {
        _setLimits(upperSwapLimit_, lowerSwapLimit_, swapFee_);
    }


    /**
     * @notice Withdraws amount from contract's supply, which is supposed to be done exclusively for relocating funds to
     *       another Bridge system, and **NO** other purpose.
     * @param targetAddress : address to send tokens to
     * @param amount : address to send tokens to
     */
    function withdraw(
        address targetAddress,
        uint256 amount
        )
        public
        onlyOwner
    {
        supply = supply.sub(amount);
        token.transfer(targetAddress, amount);
        emit Withdraw(targetAddress, amount);
    }


    /**
     * @dev Deposits funds back in to the contract supply.
     *      Dedicated to increase contract's supply, usually(but not necessarily) after previous withdrawal from supply.
     *      NOTE: This call needs preexisting ERC20 allowance >= `amount` for address of this Bridge contract as
     *            recipient/beneficiary and `from` address as sender.
     *            This means that address passed in as the `from` input parameter of this `Bridge.deposit(...)` call,
     *            must have already crated allowance by calling `ERC20.approve(from, ADDR_OF_BRIDGE_CONTRACT, amount)`
     *            *before* calling this(`deposit(...)`) call
     * @param from : address which the deposit is going to be transferred from
     * @param amount : deposit amount
     */
    function deposit(
        address from,
        uint256 amount
        )
        public
        onlyOwner
    {
        supply = supply.add(amount);
        token.transferFrom(from, address(this), amount);
        emit Deposit(msg.sender, amount);
    }


    /**
     * @dev Withdraw refunds fees accrued so far.
     * @param targetAddress : address to send tokens to.
     */
    function withdrawRefundsFees(address targetAddress)
    public
    onlyOwner
    {
        require(refundsFeesAccrued > 0, "");
        token.transfer(targetAddress, refundsFeesAccrued);
        refundsFeesAccrued = 0;
        emit RefundsFeesWithdrawal(targetAddress, refundsFeesAccrued);
    }


    /**
     * @dev Withdraw "excess" tokens (FET and ETH), which were sent to contract directly via direct transfers,
     *      (either ERC20.transfer(...) or transferring ETH), without interacting with API of this contract, what could
     *      be done only by mistake.
     *      Thus this method is meant to be used primarily for rescue purposes, enabling withdrawal of such
     *      "excess" tokens out of contract.
     *
     * @dev This call transfers also whole ETH balance present on this contract address to `targetAddress`, and
     *      forwards exactly 2300 gas stipend, what implies that `targetAddress` should not be (preferably) contract
     *      in order to avoid potential of exceeding forwarded gas stipend.
     *
     * @param targetAddress : address to send tokens to
     */
    function withdrawExcessFunds(address payable targetAddress)
        public
        onlyOwner
    {
        uint256 excessAmount = _excessFunds();
        token.transfer(targetAddress, excessAmount);
        uint256 ethBalance = address(this).balance;
        if (ethBalance > 0) {
            targetAddress.transfer(ethBalance);
        }
        emit ExcessFundsWithdrawal(targetAddress, excessAmount, ethBalance);
    }


    /**
     * @notice Delete the contract, transfers the remaining token and ether balance to the specified
     *         payoutAddress
     * @param payoutAddress address to transfer the balances to. Ensure that this is able to handle ERC20 tokens
     * @dev owner only + only on or after `earliestDelete` block
     */
    function deleteContract(address payable payoutAddress)
        public
        onlyOwner
    {
        require(earliestDelete >= _getBlockNumber(), "Earliest delete not reached");
        uint256 contractBalance = token.balanceOf(address(this));
        token.transfer(payoutAddress, contractBalance);
        emit DeleteContract(payoutAddress);
        selfdestruct(payoutAddress);
    }


    // **********************************************************
    // ******************    INTERNAL METHODS   *****************


    /**
     * @dev VIRTUAL Method returning bock number. Introduced for
     *      testing purposes (allows mocking).
     */
    function _getBlockNumber() internal view virtual returns(uint256)
    {
        return block.number;
    }


    function _isOwner() internal view returns(bool) {
        return hasRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }


    /**
     * @notice Pauses all NON-administrative interaction with the contract since the specidfed block number 
     * @param blockNumber - block number since which non-admin interaction will be paused (for all
     *                      _getBlockNumber() >= blockNumber)
     */
    function _pauseSince(uint256 blockNumber) internal 
    {
        uint256 currentBlockNumber = _getBlockNumber();
        pausedSinceBlock = blockNumber < currentBlockNumber ? currentBlockNumber : blockNumber;
        emit Pause(pausedSinceBlock);
    }


    function _setLimits(
        uint256 upperSwapLimit_,
        uint256 lowerSwapLimit_,
        uint256 swapFee_
        )
        internal
    {
        require((swapFee_ <= lowerSwapLimit_) && (lowerSwapLimit_ <= upperSwapLimit_), "fee<=lower<=upper violated");
        // NOTE(pb): No consistency checks are imposed on the configuration passed in (e.g. upperLimit >= lowerLimit,
        //  etc. ...) - this is intentional, so that desired effect can be achieved - for example temporary disabling
        //  swaps on amount base rather than pausing by setting upperLimit < lowerLimit.
        upperSwapLimit = upperSwapLimit_;
        lowerSwapLimit = lowerSwapLimit_;
        swapFee = swapFee_;

        emit LimitsUpdate(upperSwapLimit, lowerSwapLimit, swapFee);
    }


    function _setCap(uint256 cap_) internal
    {
        cap = cap_;
        emit CapUpdate(cap);
    }


    // NOTE(pb): This function shall fail(=revert) due to SafeMath,
    //           if there is inconsistency between contract balance and accrued amounts.
    function _excessFunds() internal view returns(uint256) {
        uint256 contractBalance = token.balanceOf(address(this));
        return contractBalance.sub(supply).sub(refundsFeesAccrued);
    }
}
