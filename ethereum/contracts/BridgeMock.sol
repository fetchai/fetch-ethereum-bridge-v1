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
    /// @dev This state veriable needs to be default-initialised to the current block.number value
    ///      in order to maintain the *same* behaviour as production `Bridge` contract, so the same
    ///      implementation of tests will pass for both contracts `Bridge` nad `BridgeMock`
    ///      For the very same reason, this state variable was made intentionally *private* (= so it
    ///      can *not* be used/relied-on in tests implementation).
    uint256 private blockNumber = block.number;

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
        // NOTE(pb): Changing/setting default value of `blockNumber` state variable here, inside of this
        //  *derived* contract constructor, will **NOT** have effect inside of *base* contract constructor.
        //  The reason being, that it is too late to change/set the value here, since constructor of the
        //  **BASE** contract `Bridge` has been called *already*, which internally calls the
        //  `_getBlockNumber()` method.
        //  Solidity/EVM deviates from well established concept(in other languages) where calling virtual
        //  function from within the constructor: Solidity calls override implementation from *derived*
        //  contract from withing *BASE* contract constructor execution = base class constructor call method
        //  from *derived* contract which is **NOT** initialised yet since it's constructor has **not**
        //  been called yet.
        //  Thus it can be initialised only from within the *override* implementation method itself using
        //  lazy initialisation pattern. However, the `_getBlockNumber()` method is declared as `view`,
        //  so lazy init. pattern can *not* be used there, since it would require to modify contract state.
        //  The only viable option is to use default value initialiser when the state variable is *declared*.
        //blockNumber = ...; // this will *not* have effect inside of constructor of `Bridge` base class
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