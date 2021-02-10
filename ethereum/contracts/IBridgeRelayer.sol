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
interface IBridgeRelayer {

    event SwapRefund(uint64 indexed id, address indexed to, uint256 refundedAmount, uint256 fee);
    event ReverseSwap(uint64 indexed rid, address indexed to, string indexed from, bytes32 originTxHash, uint256 effectiveAmount, uint256 fee);
    event Pause(uint256 sinceBlock);
    event NewRelayEon(uint64 eon);

    /**
      * @notice Starts the new relay eon.
      * @dev Relay eon concept is part of the design in order to ensure safe management of hand-over between two
      *      relayer services. It provides clean isolation of potentially still pending transactions from previous
      *      relayer svc and the current one.
      */
    function newRelayEon() external;


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
        external;


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
        external;


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
        external;


    /**
     * @notice Pauses all NON-administrative interaction with the contract since the specified block number
     * @param blockNumber block number since which non-admin interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pauseSince(uint256 blockNumber) external;
}
