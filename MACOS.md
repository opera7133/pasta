# macOS

Build `pasta_shiori` with Rust 1.93.0 or later:

```sh
MACOSX_DEPLOYMENT_TARGET=14.0 cargo build --locked --release -p pasta_shiori
```

The output is `target/release/libpasta.dylib`. It exports `loadu`, `load`,
`request`, and `unload` with UTF-8 strings, signed 32-bit lengths and
malloc/free ownership. The module frees inputs; the caller frees responses.

Only one session can be loaded per image. Utatane runs each ghost in a separate
process. macOS uses a portable executor for the existing actor loop and joins
the actor after shutdown before allowing the host to unload the image.

Put `libpasta.dylib` in `ghost/master/` and set
`shiori.macos,libpasta.dylib` in `descript.txt`. LuaJIT and the standard Pasta
scripts are embedded. Windows DLLs and Windows-specific Lua FFI calls cannot
run on macOS. Dictionaries and saved data follow `pasta.toml`.
