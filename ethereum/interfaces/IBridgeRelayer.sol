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


/**
 * @title *Relayer* interface of Bi-directional bridge for transfer of FET tokens between Ethereum
 *        and Fetch Mainnet-v2.
 *
 * @notice By design, all methods of this relayer-level interface can be called exclusively by relayer(s) of
 *         the Bridge contract.
 *         It is offers set of methods to perform relaying functionality of the Bridge = transferring swaps
 *         across chains.
 *
 * @notice This bridge allows to transfer [ERC20-FET] tokens from Ethereum Mainnet to [Native FET] tokens on Fetch
 *         Native Mainnet-v2 and **other way around** (= it is bi-directional).
 *         User will be *charged* swap fee defined in counterpart contract deployed on Fetch Native Mainnet-v2.
 *         In the case of a refund, user will be charged a swap fee configured in this contract.
 *
 *         Swap Fees for `swap(...)` operations (direction from this contract to Native Fetch Mainnet-v2 are handled by
 *         the counterpart contract on Fetch Native Mainnet-v2, **except** for refunds, for
 *         which user is charged swap fee defined by this contract (since relayer needs to send refund transaction back
 *         to this contract.
 */
interface IBridgeRelayer is IBridgeCommon {

    /**
      * @notice Starts the new relay eon.
      * @dev Relay eon concept is part of the design in order to ensure safe management of hand-over between two
      *      relayer services. It provides clean isolation of potentially still pending transactions from previous
      *      relayer svc and the current one.
      */
    function newRelayEon() external;


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
    function refund(uint64 id, address to, uint256 amount, uint64 relayEon_) external;


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
    function refundInFull(uint64 id, address to, uint256 amount, uint64 relayEon_) external;


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
        uint64 rid,
        address to,
        string calldata from,
        bytes32 originTxHash,
        uint256 amount,
        uint64 relayEon_
        )
        external;
}
