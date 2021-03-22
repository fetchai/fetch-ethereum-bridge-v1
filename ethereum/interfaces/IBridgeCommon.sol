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
 * @title Events for Bi-directional bridge transferring FET tokens between Ethereum and Fetch Mainnet-v2
 */
interface IBridgeCommon {

    event Swap(uint64 indexed id, address indexed from, string indexed indexedTo, string to, uint256 amount);

    event SwapRefund(uint64 indexed id, address indexed to, uint256 refundedAmount, uint256 fee);
    event ReverseSwap(uint64 indexed rid, address indexed to, string indexed from, bytes32 originTxHash, uint256 effectiveAmount, uint256 fee);
    event Pause(uint256 sinceBlock);
    event NewRelayEon(uint64 eon);

    event LimitsUpdate(uint256 max, uint256 min, uint256 fee);
    event CapUpdate(uint256 value);
    event ReverseAggregateAllowanceUpdate(uint256 value);
    event Withdraw(address indexed targetAddress, uint256 amount);
    event Deposit(address indexed fromAddress, uint256 amount);
    event FeesWithdrawal(address indexed targetAddress, uint256 amount);
    event DeleteContract(address targetAddress, uint256 amount);
    // NOTE(pb): It is NOT necessary to have dedicated events here for Mint & Burn operations, since ERC20 contract
    //  already emits the `Transfer(from, to, amount)` events, with `from`, resp. `to`, address parameter value set to
    //  ZERO_ADDRESS (= address(0) = 0x00...00) for `mint`, resp `burn`, calls to ERC20 contract. That way we can
    //  identify events for mint, resp. burn, calls by filtering ERC20 Transfer events with `from == ZERO_ADDR  &&
    //  to == Bridge.address` for MINT operation, resp `from == Bridge.address` and `to == ZERO_ADDR` for BURN operation.
    //event Mint(uint256 amount);
    //event Burn(uint256 amount);

    function getDelegateRole() external view returns(bytes32);
    function getRelayerRole() external view returns(bytes32);

    function getToken() external view returns(address);
    function getEarliestDelete() external view returns(uint256);
    function getSupply() external view returns(uint256);
    function getNextSwapId() external view returns(uint64);
    function getRelayEon() external view returns(uint64);
    function getRefund(uint64 swap_id) external view returns(uint256); // swapId -> original swap amount(= *includes* swapFee)
    function getSwapMax() external view returns(uint256);
    function getSwapMin() external view returns(uint256);
    function getCap() external view returns(uint256);
    function getSwapFee() external view returns(uint256);
    function getPausedSinceBlock() external view returns(uint256);
    function getReverseAggregateAllowance() external view returns(uint256);
}
