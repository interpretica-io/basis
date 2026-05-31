# basis

A small **constellation build system** written in Rust. A *constellation* is a
group of related repositories that are built, cleaned and versioned together.

`basis` can:

1. **Build constellations** of repositories driven by a single YAML manifest,
   with per-action command sets (`build`, `clean`, or any custom action).
2. **Track versions** per repository — Rust via `Cargo.toml`, C++ via a
   `.version` file (and/or `CMakeLists.txt`).
3. **Synchronise versions** across every repository to one common value.
4. **Report status** — git state plus version-sync state of the whole
   constellation via `basis status`.

## Install

```sh
cargo install --path .
# or run from the workspace:
cargo run -- <args>
```

## The manifest (`basis.yaml`)

```yaml
constellation: my-product
version: 1.2.0            # optional canonical version of the constellation

repos:
  - name: core           # unique name, used with --repo
    path: core           # path relative to the manifest
    lang: rust           # rust | cpp
    actions:
      build: [cargo build --release]
      clean: [cargo clean]

  - name: engine
    path: engine
    lang: cpp
    version_file: .version       # optional, default: .version
    cmake_file: CMakeLists.txt   # optional, default: CMakeLists.txt
    actions:
      build:
        - cmake -B build -S .
        - cmake --build build
      clean: [rm -rf build]
```

* Each repo defines a map of **action → ordered shell commands**. Commands run
  in the repo's directory via `sh -c`.
* `actions` keys are arbitrary; `build` and `clean` get dedicated subcommands,
  anything else is reachable through `basis run <action>`.

## Commands

```sh
basis build [--repo NAME]... [-k] [-n]   # run the `build` action
basis clean [--repo NAME]... [-k] [-n]   # run the `clean` action
basis run <action> [--repo NAME]...      # run any named action

basis status                             # git + version status of all repos

basis version                            # alias of `version show`
basis version show                       # list every repo's version
basis version set <X.Y.Z>                # set an explicit version everywhere
basis version sync [--to <X.Y.Z>]        # converge all repos onto one version
```

Common flags:

* `-f, --file <PATH>` — manifest path (default `basis.yaml`).
* `-r, --repo <NAME>` — restrict to specific repos (repeatable).
* `-k, --keep-going` — continue across repos even if one command fails.
* `-n, --dry-run` — print commands without executing them.

### Version sync target

`basis version sync` chooses its target in this order:

1. `--to <X.Y.Z>` if given,
2. otherwise the manifest's top-level `version:`,
3. otherwise the highest semver found among the repositories.

For Rust repos it rewrites `[package].version` in `Cargo.toml` (preserving
formatting). For C++ repos it writes the `.version` file and patches the
`project(... VERSION x.y.z ...)` call in the CMake file when present.

## Example

A runnable example lives in [`examples/`](examples/). From there:

```sh
basis -f examples/basis.yaml status
basis -f examples/basis.yaml version sync
basis -f examples/basis.yaml build -n
```
