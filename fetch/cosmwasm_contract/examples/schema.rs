use std::env::current_dir;
use std::fs::create_dir_all;

use cosmwasm_schema::{export_schema, remove_schemas, schema_for};

use bridge::msg::{
    CapResponse, DenomResponse, ExecuteMsg, InstantiateMsg, PausedSinceBlockResponse, QueryMsg,
    RelayEonResponse, ReverseAggregatedAllowanceResponse, RoleResponse, SupplyResponse,
    SwapMaxResponse,
};

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    export_schema(&schema_for!(InstantiateMsg), &out_dir);
    export_schema(&schema_for!(ExecuteMsg), &out_dir);
    export_schema(&schema_for!(QueryMsg), &out_dir);
    export_schema(&schema_for!(RoleResponse), &out_dir);
    export_schema(&schema_for!(RelayEonResponse), &out_dir);
    export_schema(&schema_for!(SupplyResponse), &out_dir);
    export_schema(&schema_for!(PausedSinceBlockResponse), &out_dir);
    export_schema(&schema_for!(DenomResponse), &out_dir);
    export_schema(&schema_for!(CapResponse), &out_dir);
    export_schema(&schema_for!(SwapMaxResponse), &out_dir);
    export_schema(&schema_for!(ReverseAggregatedAllowanceResponse), &out_dir);
}
