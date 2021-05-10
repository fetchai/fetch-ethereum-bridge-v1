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
pragma abicoder v2;

import "../openzeppelin/contracts/access/AccessControl.sol";

contract MultisigCoordinator is AccessControl {

    address[] public committee;
    mapping (address => uint8) public committeeMap;
    uint8 public threshold; // immutable state

    // @notice signature type (= value type in mapping) is string in order to make it general - client can pass JSON
    mapping (address => string) public signaturesMap;
    // @notice `signees` array of addresses which signed
    address[] signees;

    // @notice `action` should be string to make it general
    string public action;
    // @notice `actionData` are intentionally string to make it general - client can pass JSON formatted string
    string public actionData;
    // @notice `coordinationNonce` represents unique zero-based identifier for action proposal
    uint64 public coordinationNonce;

    // @notice `timeout` in number of blocks
    uint256 timeout;

    uint256 expiredSinceBlock;


    modifier isAdmin() {
        require(hasRole(DEFAULT_ADMIN_ROLE, msg.sender), "Only admin role");
        _;
    }


    modifier isMember() {
        require(committeeMap[msg.sender] > 0, "not committee member");
        _;
    }


    modifier verifyCoordinationId(uint64 coordinationNonce_) {
        require(coordinationNonce == coordinationNonce_, "coordinationNonce mismatch");
        _;
    }


    modifier isExpired() {
        require(block.number >= expiredSinceBlock, "expiration block NOT reached");
        _;
    }


    constructor(
          address[] memory committee_
        , uint8 threshold_
        , uint256 timeout_
        )
    {
        require(/*committee_.length <= 10 && */threshold_ <= committee_.length && threshold_ > 0 && committee_.length < 64);

        _setupRole(DEFAULT_ADMIN_ROLE, msg.sender);
        _configure(committee_, threshold_, timeout_);
    }


    function configure(
          address[] memory committee_
        , uint8 threshold_
        , uint256 timeout_
        )
        external
        isAdmin
    {
        _configure(committee_, threshold_, timeout_);
    }


    function propose(
        uint64 coordinationNonce_,
        string calldata action_,
        string calldata actionData_,
        string calldata signature_
        )
        external
        isMember
        isExpired
        verifyCoordinationId(coordinationNonce_)
    {
        require(bytes(action_).length > 0, "empty action");

        _deleteSignatures();

        ++coordinationNonce;
        // @notice In theory this can overflow, practically it won't. And even if it would nothing would happen
        expiredSinceBlock = block.number + timeout;

        action = action_;
        actionData = actionData_;

        _sign(signature_);
    }


    function sign(uint64 coordinationNonce_, string calldata signature)
        external
        isMember
        verifyCoordinationId(coordinationNonce_)
    {
        _sign(signature);
    }


    function getCommitteeSize() external view returns(uint256) {
        return committee.length;
    }


    function getSigneesSize() external view returns(uint256) {
        return signees.length;
    }


    function getCoordinationSetup()
        external
        view
        returns(uint64 coordinationNonce_,
                uint8 threshold_,
                uint256 timeout_,
                address[] memory committee_
                )
    {
        coordinationNonce_ = coordinationNonce;
        threshold_ = threshold;
        timeout_ = timeout;
        committee_ = new address[] (committee.length);
        for (uint256 i=0; i<committee.length; ++i) {
            committee_[i] = committee[i];
        }
    }


    function getSigningState()
        external
        view
        returns(uint64 coordinationNonce_,
                string memory action_,
                string memory actionData_,
                uint256 expiredSinceBlock_,
                address[] memory signees_,
                string[] memory signatures_)
    {
        coordinationNonce_ = coordinationNonce;
        action_ = action;
        actionData_ = actionData;
        expiredSinceBlock_ = expiredSinceBlock;
        signees_ = new address[] (signees.length);
        signatures_ = new string[](signees.length);
        for (uint256 i=0; i<signees.length; ++i) {
            signees_[i] = signees[i];
            signatures_[i] = signaturesMap[signees[i]];
        }
    }


    function _sign(string calldata signature) internal
    {
        require(bytes(signaturesMap[msg.sender]).length == 0, "member already signed");
        signaturesMap[msg.sender] = signature;
        signees.push(msg.sender);
    }


    function _configure(
          address[] memory committee_
        , uint8 threshold_
        , uint256 timeout_
        )
        internal
    {
        require(/*committee_.length <= 10 && */threshold_ <= committee_.length && threshold_ > 0 && committee_.length < 64);

        _deleteCommittee();
        _deleteSignatures();

        for (uint i = 0; i < committee_.length; i++) {
            committeeMap[committee_[i]] = uint8(i+1);
        }

        committee = committee_;
        threshold = threshold_;

        action = "";
        actionData = "";
        ++coordinationNonce;

        timeout = timeout_;
        expiredSinceBlock = 0;
    }


    function _deleteCommittee() internal {
        // @notice Theoretically vulnerable to out-of-gas issue is array if huge, but practically this will
        //         never happen, since committee size will be always limited due to practical reasons.
         if (committee.length > 0) {
            for (uint i = 0; i < committee.length; i++) {
                delete committeeMap[committee[i]];
            }
            delete committee;
        }
    }


    function _deleteSignatures() internal {
        // @notice Theoretically vulnerable to out-of-gas issue is array if huge, but practically this will
        //         never happen, since committee size will be always limited due to practical reasons.
         if (signees.length > 0) {
            for (uint i = 0; i < signees.length; i++) {
                delete signaturesMap[signees[i]];
            }
            delete signees;
        }
    }
}
