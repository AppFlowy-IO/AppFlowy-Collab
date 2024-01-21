
## Run clippy for web

```shell
cargo clippy --target=wasm32-unknown-unknown --fix --allow-dirty --features="wasm_build"
```

## Run tests in Chrome
```shell
wasm-pack test --chrome --features="wasm_build"
```

## Build for web

```shell
wasm-pack build --features="wasm_build"
```