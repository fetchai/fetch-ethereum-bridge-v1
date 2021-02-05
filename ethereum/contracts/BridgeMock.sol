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

import "./Bridge.sol";


contract BridgeMock is Bridge
{
    uint256 public blockNumber;


    constructor(
          address ERC20Address
        , uint256 cap_
        , uint256 upperSwapLimit_
        , uint256 lowerSwapLimit_
        , uint256 swapFee_
        , uint64 pausedSinceBlock_
        , uint64 deleteProtectionPeriod_
        )
        Bridge(
          ERC20Address
        , cap_
        , upperSwapLimit_
        , lowerSwapLimit_
        , swapFee_
        , pausedSinceBlock_
        , deleteProtectionPeriod_
        )
    {
        blockNumber = block.number;
    }


    function setBlockNumber(uint256 value) public virtual
    {
        blockNumber = value;
    }

    function _getBlockNumber() internal view override returns(uint256)
    {
        return blockNumber;
    }
}
