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
interface IBridgePublic {

    event Swap(uint64 indexed id, string indexed indexedTo, string to, uint256 amount);

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
        external;
}
