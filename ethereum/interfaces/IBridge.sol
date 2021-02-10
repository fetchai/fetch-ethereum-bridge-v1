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

import "./IBridgePublic.sol";
import "./IBridgeRelayer.sol";
import "./IBridgeAdmin.sol";


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
interface IBridge is IBridgePublic, IBridgeRelayer, IBridgeAdmin {}
