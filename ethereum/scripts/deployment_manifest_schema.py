from dataclasses import dataclass, field
from dataclasses_json import dataclass_json, config
from typing import Optional


@dataclass_json
@dataclass
class ContractConstructorParamsBase:
    pass


@dataclass_json
@dataclass
class ContractParamsBase:
    deployer_address: Optional[str]
    deployer_public_key: Optional[str]
    contract_address: Optional[str]
    constructor_parameters: ContractConstructorParamsBase


@dataclass_json
@dataclass
class FetERC20MockConstructorParams(ContractConstructorParamsBase):
    name: str
    symbol: str
    initialSupply: int
    decimals_: int


@dataclass_json
@dataclass
class FetERC20MockParams(ContractParamsBase):
    constructor_parameters: FetERC20MockConstructorParams


@dataclass_json
@dataclass
class BridgeConstructorParams(ContractConstructorParamsBase):
    ERC20Address: str
    cap: int
    upperSwapLimit: int
    lowerSwapLimit: int
    swapFee: int
    pausedSinceBlock: int = field(
        metadata=config(
            encoder=lambda value: hex(value),
            decoder=lambda value: int(value, 0)
        ))
    deleteProtectionPeriod: int


@dataclass_json
@dataclass
class Account:
    address: str
    funding: Optional[int] = None


@dataclass_json
@dataclass
class BridgeParams(ContractParamsBase):
    admin_wallet: Account
    relayer_wallet: Account
    constructor_parameters: BridgeConstructorParams


@dataclass_json
@dataclass
class NetworkManifest:
    FetERC20Mock: Optional[FetERC20MockParams]
    Bridge: Optional[BridgeParams]
