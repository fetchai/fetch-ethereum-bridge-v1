
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

pragma solidity ^0.8.0;

import "./IBridgeCommon.sol";


/**
 * @title *Monitor* interface of Bi-directional bridge for transfer of FET tokens between Ethereum
 *        and Fetch Mainnet-v2.
 *
 * @notice By design, all methods of this monitor-level interface can be called monitor and admin roles of
 *         the Bridge contract.
 *
 */
interface IBridgeMonitor is IBridgeCommon {
    /**
     * @notice Pauses Public API since the specified block number
     * @param blockNumber block number since which non-admin interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pausePublicApiSince(uint256 blockNumber) external;

    /**
     * @notice Pauses Relayer API since the specified block number
     * @param blockNumber block number since which non-admin interaction will be paused (for all
     *        block.number >= blockNumber).
     * @dev Delegate only
     *      If `blocknumber < block.number`, then contract will be paused immediately = from `block.number`.
     */
    function pauseRelayerApiSince(uint256 blockNumber) external;
}
