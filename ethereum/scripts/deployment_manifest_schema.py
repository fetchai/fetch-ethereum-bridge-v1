from dataclasses import dataclass, field
from dataclasses_json import dataclass_json, config
from typing import Optional


int_hex_metadata_config = config(
            encoder=lambda value: hex(value),
            decoder=lambda value: value if isinstance(value, int) else int(value, 0)
        )

int_int_metadata_config = config(
            decoder=lambda value: value if isinstance(value, int) else int(value, 0)
        )

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
    initialSupply: int = field(
        metadata=int_int_metadata_config)
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
    reverseAggregateAllowance: int = field(
        metadata=int_int_metadata_config)

    upperSwapLimit: int = field(
        metadata=int_int_metadata_config)

    lowerSwapLimit: int = field(
        metadata=int_int_metadata_config)

    swapFee: int = field(
        metadata=int_int_metadata_config)

    pausedSinceBlock: int = field(
        metadata=int_hex_metadata_config)

    deleteProtectionPeriod: int = field(
        metadata=int_int_metadata_config)



@dataclass_json
@dataclass
class Account:
    address: str
    funding: Optional[int] = field(
        default=None,
        metadata=int_int_metadata_config)



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
