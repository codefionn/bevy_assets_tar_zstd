# bevy_assets_tar_zstd

This package is for packaging assets in bevy in a `tar.zstd` file at compile-time.

## Automatically package assets

Package assets in the `assets` directory in the assets.tar.zstd file at built-time:

```rust
fn main() {
    bevy_assets_tar_zstd_bundler::bundle_asset(bevy_assets_tar_zstd_bundler::Config::default());
}
```

## Add asset loader

```rust
App::new()
    .add_plugin(bevy_assets_tar_zstd::AssetsTarZstdPlugin::default())
```
