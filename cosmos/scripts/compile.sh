cd $1
RUSTFLAGS='-C link-arg=-s' cargo wasm
cp target/wasm32-unknown-unknown/release/*.wasm ../


