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
    publish_source: Optional[bool]
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

    cap: int = field(
        metadata=int_int_metadata_config)

    reverseAggregatedAllowance: int = field(
        metadata=int_int_metadata_config)

    reverseAggregatedAllowanceApproverCap: int = field(
        metadata=int_int_metadata_config)

    swapMax: int = field(
        metadata=int_int_metadata_config)

    swapMin: int = field(
        metadata=int_int_metadata_config)

    reverseSwapMax: int = field(
        metadata=int_int_metadata_config)

    reverseSwapMin: int = field(
        metadata=int_int_metadata_config)

    reverseSwapFee: int = field(
        metadata=int_int_metadata_config)

    pausedSinceBlockPublicApi: int = field(
        metadata=int_hex_metadata_config)

    pausedSinceBlockRelayerApi: int = field(
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
    admin_wallet: Optional[Account]
    relayer_wallet: Optional[Account]
    monitor_wallet: Optional[Account]
    approver_wallet: Optional[Account]
    constructor_parameters: BridgeConstructorParams


@dataclass_json
@dataclass
class NetworkManifest:
    FetERC20Mock: Optional[FetERC20MockParams]
    Bridge: Optional[BridgeParams]
