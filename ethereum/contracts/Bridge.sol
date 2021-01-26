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

import "../openzeppelin/contracts/token/ERC20/IERC20.sol";
import "../openzeppelin/contracts/access/AccessControl.sol";
import "../openzeppelin/contracts/math/SafeMath.sol";


contract Bridge is AccessControl {
    using SafeMath for uint256;

    // *******    USER-LEVEL EVENTS    ********
    event Swap(uint256 indexed id, string indexed to, uint256 amount, uint256 fee);
    // *******    DELEGATE-LEVEL EVENTS    ********
    event SwapRefund(uint256 indexed id);
    event ReverseSwap(uint256 indexed rid, address indexed to, string indexed from, bytes32 originTxHash, uint256 effectiveAmount);
    event Pause(uint256 sinceBlock);
    // *******    ADMIN-LEVEL EVENTS    ********
    event LimitsUpdate(uint256 upperSwqpLimit, uint256 lowerSwapLimit, uint256 swapFee);
    event CapUpdate(uint256 amount);
    event Inflate(uint256 amount);
    event Deflate(uint256 amount);
    event FeesWithdrawal(address targetAddress, uint256 amount);
    event ExcessFundsWithdrawal(address targetAddress, uint256 amount);
    event DeleteContract(address payoutAddress);

    bytes32 public constant DELEGATE_ROLE = keccak256("DELEGATE_ROLE");


    // *******    STATE    ********
    IERC20 public token;
    uint256 public effectiveAmountAccrued;
    uint256 public inflationAmount;
    uint256 public feesAccrued;
    uint256 public nextSwapId;
    //uint256 public nextReverseSwapId;
    mapping(uint256 => uint256) refunds; // swapId -> effectiveAmount
    uint256 public upperSwapLimit;
    uint256 public lowerSwapLimit;
    uint256 public cap;
    uint256 public swapFee;
    uint256 public pausedSinceBlock;
    uint256 public immutable earliestDelete;


    /* Only callable by owner */
    modifier onlyOwner() {
        require(_isOwner(), "Caller is not an owner");
        _;
    }

    /* Only callable by owner or delegate */
    modifier onlyDelegate() {
        require(_isOwner() || hasRole(DELEGATE_ROLE, msg.sender), "Caller must be owner or delegate");
        _;
    }

    modifier verifyTxExpiration(uint256 expirationBlock) {
        require(_getBlockNumber() <= expirationBlock, "Transaction expired");
        _;
    }

    modifier verifyNotPaused() {
        require(pausedSinceBlock > _getBlockNumber(), "Contract has been paused");
        _;
    }

    modifier verifySwapAmount(uint256 amount) {
        require(amount > swapFee, "Amount must be higher than fee");
        require(amount >= lowerSwapLimit, "Amount bellow lower limit");
        require(amount <= upperSwapLimit, "Amount exceeds upper limit");
        _;
    }

    modifier verifySwapId(uint256 id) {
        require(id < nextSwapId, "Invalid swap id");
        _;
    }


    /*******************
    Contract start
    *******************/
    /**
     * @param ERC20Address address of the ERC20 contract
     * @param cap_ address of the ERC20 contract
     * @param upperSwapLimit_ value representing UPPER limit which can be transferred (this value INCLUDES swapFee)
     * @param lowerSwapLimit_ value representing LOWER limit which can be transferred (this value INCLUDES swapFee)
     * @param swapFee_ represents fee which user has to pay for swap execution,
     * @param pausedSinceBlock_ block number since which the contract will be paused for all user-level actions
     * @param deleteProtectionPeriod_ number of blocks(from contract deployment block) during which contract can NOT be deleted
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
        token = IERC20(ERC20Address);
        earliestDelete = _getBlockNumber().add(deleteProtectionPeriod_);

        // NOTE(pb): Unnecessary initialisations, done implicitly by VM
        //effectiveAmountAccrued = 0;
        //feesAccrued = 0;
        //nextSwapId = 0;
        //inflationAmount = 0;

        _setCap(cap_);
        _setLimits(upperSwapLimit_, lowerSwapLimit_, swapFee_);
        _pauseSince(pausedSinceBlock_);
    }


    // **********************************************************
    // ***********    USER-LEVEL ACCESS METHODS    **********


    function swap(
        uint256 amount, // This is original amount (INCLUDES fee)
        string calldata destinationAddress
        )
        public
        verifyNotPaused
        verifySwapAmount(amount)
    {
        uint256 effectiveAmount = amount.sub(swapFee);
        effectiveAmountAccrued = effectiveAmountAccrued.add(effectiveAmount);
        require(cap >= effectiveAmountAccrued, "Swap would exceed cap");
        require(token.transferFrom(msg.sender, address(this), amount), "Allowance too low");

        feesAccrued = feesAccrued.add(swapFee);

        emit Swap(nextSwapId, destinationAddress, amount, swapFee);

        // NOTE(pb): NO safe math necessary here:
        nextSwapId += 1;
    }


    function getExcessFunds() public view returns(uint256) {
        return _excessFunds();
    }


    // **********************************************************
    // ***********    DELEGATE-LEVEL ACCESS METHODS    **********


    function refund(
        uint256 id,
        address to,
        uint256 effectiveAmount, // This is WITHOUT fee = original amount - fee
        uint256 txExpirationBlock
        )
        public
        onlyDelegate
        verifySwapId(id)
        verifyTxExpiration(txExpirationBlock)
    {
        require(refunds[id] == 0, "Refund was already processed");
        require(token.transfer(to, effectiveAmount), "Transfer failed");
        refunds[id] = effectiveAmount;
        effectiveAmountAccrued = effectiveAmountAccrued.sub(effectiveAmount);
        emit SwapRefund(id);
    }

    // NOTE(pb):  Fee is *NOT* refunded back to the user (this is by design)
    function reverseSwap(
        uint256 rid, // Reverse swp id (from counterpart contract on other blockchain)
        address to,
        string calldata from,
        bytes32 originTxHash,
        uint256 effectiveAmount, // This shall be effectiveAmount (WITHOUT fee) = originalAmount - fee
        uint256 txExpirationBlock
        )
        public
        onlyDelegate
        verifyTxExpiration(txExpirationBlock)
    {
        require(token.transfer(to, effectiveAmount), "Transfer failed");
        effectiveAmountAccrued = effectiveAmountAccrued.sub(effectiveAmount);
        emit ReverseSwap(rid, to, from, originTxHash, effectiveAmount);
    }


    /**
     * @notice Pauses all NON-administrative interaction with the contract since the specidfed block number 
     * @param blockNumber block number since which non-admin interaction will be paused (for all _getBlockNumber() >= blockNumber)
     * @dev Delegate only
     */
    function pauseSince(uint256 blockNumber, uint256 txExpirationBlock)
        public
        verifyTxExpiration(txExpirationBlock)
        onlyDelegate
    {
        _pauseSince(blockNumber);
    }


    // **********************************************************
    // ************    ADMIN-LEVEL ACCESS METHODS   *************


    function inflate(uint256 amount)
        public
        onlyOwner
    {
        inflationAmount = inflationAmount.add(amount);

        // NOTE(pb): This needs to be done, so inflationary amount of tokens from counterpart blockchain corresponding
        //  to `effectiveAmountAccrued` *ORIGINALLY* transferred in to that blockchain (= before inflation) can be
        //  transferred back to the *source*(=Ethereum) blockchain.
        //  Please keep in mind that this action **SHALL** be preceded with **MINTING** of the `amount`
        //  of ERC20 FET tokens.
         effectiveAmountAccrued = effectiveAmountAccrued.add(amount);
        emit Inflate(amount);

        // NOTE(pb): We should think if alignment of cap is actually right thing to do here.
        _setCap(cap.add(amount));
    }


    function deflate(uint256 amount)
        public
        onlyOwner
    {
        // NOTE(pb): any of following subtractions will fail should there be insufficient value
        //  on any of state variables bellow.

        inflationAmount = inflationAmount.sub(amount);
        // NOTE(pb): The `effectiveAmountAccrued` shall be adjusted when inflation is removed from the system (when
        //  the inflation is *reduced* in counterpart blockchain).
        //  This is to **PREVENT** transfer of more inflationary tokens back to *source*(=Ethereum) blockchain
        //than it was originally(= before inflation/deflation) transferred to the counterpart blockchain.
        //  Please keep in mind that this action **SHALL** be preceded with *BURNING* of the `amount`
        //  of ERC20 FET tokens.
        effectiveAmountAccrued = effectiveAmountAccrued.sub(amount);

        emit Deflate(amount);

        // NOTE(pb): We should think if alignment of cap is actually right thing to do here.
        if (cap < amount)
        {
            cap = 0;
        }
        else
        {
            _setCap(cap.sub(amount));
        }
    }


    function setCap(uint256 cap_)
        public
        onlyOwner
    {
        _setCap(cap_);
    }


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


    function withdrawFees(address targetAddress)
        public
        onlyOwner
    {
        require(token.transfer(targetAddress, feesAccrued), "Transfer failed");
        emit FeesWithdrawal(targetAddress, feesAccrued);
        feesAccrued = 0;
    }


    /**
     * @dev Withdraw "excess" tokens, which were sent to contract directly via direct ERC20.transfer(...),
     *      without interacting with API of this (Staking) contract, what could be done only by mistake.
     *      Thus this method is meant to be used primarily for rescue purposes, enabling withdrawal of such
     *      "excess" tokens out of contract.
     * @param targetAddress : address to send tokens to
     */
    function withdrawExcessTokens(address payable targetAddress)
        public
        onlyOwner
    {
        uint256 excessAmount = _excessFunds();
        require(token.transfer(targetAddress, excessAmount), "Transfer failed");
        emit ExcessFundsWithdrawal(targetAddress, excessAmount);
    }


    /**
     * @notice Delete the contract, transfers the remaining token and ether balance to the specified
       payoutAddress
     * @param payoutAddress address to transfer the balances to. Ensure that this is able to handle ERC20 tokens
     * @dev owner only + only on or after `earliestDelete` block
     */
    function deleteContract(address payable payoutAddress)
        external
        onlyOwner
    {
        require(earliestDelete >= _getBlockNumber(), "Earliest delete not reached");
        uint256 contractBalance = token.balanceOf(address(this));
        require(token.transfer(payoutAddress, contractBalance));
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
     * @param blockNumber block number since which non-admin interaction will be paused (for all _getBlockNumber() >= blockNumber)
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
        return contractBalance.sub(effectiveAmountAccrued).sub(feesAccrued);
    }
}
