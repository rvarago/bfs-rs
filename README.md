# bfs-rs

A toy FUSE view over a cloud-hosted storage bucket.

## Disclaimer

> This is just a toy project of mine, only meant to serve as something fun for me to build with Rust.
> For the time being, I do **not** intend to maintain nor make it production-ready.

## Description

`bfs-rs` is a simple readonly, "Filesystem in Userspace" (FUSE) where data gets fetched from a cloud-storage through a backend.

## Configuration

So long as the extension matches a format supported by the [config crate](https://github.com/mehcode/config-rs), the user may pick whatever one likes. As an example, here's the schema informally defined as a TOML document:

```toml
[source]
bucket = "<string>"

[filesystem]
mountpoint = "<path>"

[backend]
provider = "<aws>"
endpoint = "<uri>" # optional
```

## Backends

- [AWS S3](https://aws.amazon.com/s3/).

## Examples

TODO

## Wish-list

- Hide backends behind feature flags.

## Instructions

> Dependencies:
>
> - Minimum Supported Rust Version (MSRV): `1.58.0`,
> - FUSE utilities as documented in the [fuser crate](https://github.com/cberner/fuser#dependencies),
> - A Linux box (I've only tried it on Linux boxes).

- Lint

```sh
cargo clippy
```

- Build

```sh
cargo build
```

- Test

```sh
cargo test
```
