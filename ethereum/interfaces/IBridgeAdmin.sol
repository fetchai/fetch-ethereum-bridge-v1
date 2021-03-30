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

import "./IBridgeCommon.sol";
import "./IBridgeMonitor.sol";


/**
 * @title *Administrative* interface of Bi-directional bridge for transfer of FET tokens between Ethereum
 *        and Fetch Mainnet-v2.
 *
 * @notice By design, all methods of this administrative interface can be called exclusively by administrator(s) of
 *         the Bridge contract, since it allows to configure essential parameters of the the Bridge, and change
 *         supply transferred across the Bridge.
 */
interface IBridgeAdmin is IBridgeCommon, IBridgeMonitor {
    /**
     * @notice Mints provided amount of FET tokens.
     *         This is to reflect changes in minted Native FET token supply on the Fetch Native Mainnet-v2 blockchain.
     * @param amount - number of FET tokens to mint.
     */
    function mint(uint256 amount) external;


    /**
     * @notice Burns provided amount of FET tokens.
     *         This is to reflect changes in minted Native FET token supply on the Fetch Native Mainnet-v2 blockchain.
     * @param amount - number of FET tokens to burn.
     */
    function burn(uint256 amount) external;


    /**
     * @notice Sets cap (max) value of `supply` this contract can hold = the value of tokens transferred to the other
     *         blockchain.
     *         This cap affects(limits) all operations which *increase* contract's `supply` value = `swap(...)` and
     *         `mint(...)`.
     * @param value - new cap value.
     */
    function setCap(uint256 value) external;


    /**
     * @notice Sets value of `reverseAggregatedAllowance` state variable.
     *         This affects(limits) operations which *decrease* contract's `supply` value via **RELAYER** authored
     *         operations (= `reverseSwap(...)` and `refund(...)`). It does **NOT** affect **ADMINISTRATION** authored
     *         supply decrease operations (= `withdraw(...)` & `burn(...)`).
     * @param value - new cap value.
     */
    function setReverseAggregatedAllowance(uint256 value) external;

    /**
     * @notice Sets value of `reverseAggregatedAllowanceCap` state variable.
     *         This limits APPROVER_ROLE from top - value up to which can approver rise the allowance.
     * @param value - new cap value (absolute)
     */
    function setReverseAggregatedAllowanceApproverCap(uint256 value) external;


    /**
     * @notice Sets limits for swap amount
     *         FUnction will revert if following consitency check fails: `swapfee_ <= swapMin_ <= swapMax_`
     * @param swapMax_ : >= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param swapMin_ : <= swap amount, applies for **OUTGOING** swap (= `swap(...)` call)
     * @param swapFee_ : defines swap fee for **INCOMING** swap (= `reverseSwap(...)` call), and `refund(...)`
     */
    function setLimits(uint256 swapMax_, uint256 swapMin_, uint256 swapFee_) external;


    /**
     * @notice Withdraws amount from contract's supply, which is supposed to be done exclusively for relocating funds to
     *       another Bridge system, and **NO** other purpose.
     * @param targetAddress : address to send tokens to
     * @param amount : amount of tokens to withdraw
     */
    function withdraw(address targetAddress, uint256 amount) external;


    /**
     * @dev Deposits funds back in to the contract supply.
     *      Dedicated to increase contract's supply, usually(but not necessarily) after previous withdrawal from supply.
     *      NOTE: This call needs preexisting ERC20 allowance >= `amount` for address of this Bridge contract as
     *            recipient/beneficiary and Tx sender address as sender.
     *            This means that address passed in as the Tx sender, must have already crated allowance by calling the
     *            `ERC20.approve(from, ADDR_OF_BRIDGE_CONTRACT, amount)` *before* calling this(`deposit(...)`) call.
     * @param amount : deposit amount
     */
    function deposit(uint256 amount) external;


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
    function withdrawFees(address targetAddress) external;


    /**
     * @notice Delete the contract, transfers the remaining token and ether balance to the specified
     *         payoutAddress
     * @param targetAddress address to transfer the balances to. Ensure that this is able to handle ERC20 tokens
     * @dev owner only + only on or after `earliestDelete` block
     */
    function deleteContract(address payable targetAddress) external;
}
