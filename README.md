# Rjv

A proof of concept live-coding VST. Needs a monkey-patched version of `nih_plug`, see:

- https://github.com/kelleyvanevert/nih-plug/tree/string_param

## Building

After installing [Rust](https://rustup.rs/), you can compile Rjv as follows:

```shell
cargo xtask bundle rjv --release
```

My one-liner:

```sh
cargo xtask bundle rjv --release && cp -r target/bundled/* ~/vst
```
