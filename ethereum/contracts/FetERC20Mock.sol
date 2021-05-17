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

import "@openzeppelin/mocks/ERC20Mock.sol";
import "@openzeppelin/token/ERC20/extensions/ERC20Burnable.sol";
import "@openzeppelin/token/ERC20/extensions/ERC20Pausable.sol";
import "@openzeppelin/access/AccessControlEnumerable.sol";


contract FetERC20Mock is ERC20Mock, ERC20Burnable, ERC20Pausable, AccessControlEnumerable
{
    constructor (
        string memory name,
        string memory symbol,
        uint256 initialSupply
        )
        payable
        ERC20Mock(name, symbol, msg.sender, initialSupply)
    {
        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
    }

    function _beforeTokenTransfer(address from, address to, uint256 amount) internal virtual override(ERC20, ERC20Pausable) {
        ERC20Pausable._beforeTokenTransfer(from, to, amount);
    }

    function pause() public {
        _pause();
    }

    function unpause() public {
        _unpause();
    }
}
