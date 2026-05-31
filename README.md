# basis

A small **constellation build system** written in Rust. A *constellation* is a
group of related repositories that are built, cleaned and versioned together.

`basis` can:

1. **Build constellations** of repositories driven by a single YAML manifest,
   with per-action command sets (`build`, `clean`, or any custom action).
2. **Track versions** per repository — Rust via `Cargo.toml`, C++ via a
   `.version` file (and/or `CMakeLists.txt`).
3. **Synchronise versions** across every repository to one common value.
4. **Bump one component** and propagate the new version into every repository
   that depends on it.
5. **Verify identity** — check that each repo's `git config user.email` and the
   e-mail on its GPG signing key are on an allowed domain.
6. **Report status** — git state plus version-sync state of the whole
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
email_domain: corp.com   # optional identity policy (see `basis verify`)

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
    provides: core               # optional package name exposed to dependents
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
basis verify                             # check git/GPG e-mail domains

basis version                            # alias of `version show`
basis version show                       # list every repo's version
basis version set <X.Y.Z>                # set an explicit version everywhere
basis version sync [--to <X.Y.Z>]        # converge all repos onto one version
basis version bump <repo> [--major|--minor|--patch|--to X.Y.Z]
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

### Bumping a component and its dependents

`basis version bump <repo>` raises one component's version (default `--patch`,
or `--major` / `--minor` / `--to X.Y.Z`) and then rewrites every repository that
depends on it:

* **Rust dependents** — the matching entry in `[dependencies]`,
  `[dev-dependencies]` or `[build-dependencies]` gets its `version` updated,
  keeping `path`, features and `package =` renames intact.
* **C++ dependents** — `find_package(<name> <ver> ...)` in the CMake file is
  re-pinned.

Matching is by the bumped repo's **provided name**: its `provides:` field if
set, otherwise the Rust crate name (`[package].name`), otherwise the repo name.

```sh
$ basis version bump core --minor
bumping core 1.0.0 -> 1.1.0 (provides 'core')
  ✓ core version set to 1.1.0
  ↳ app now requires core 1.1.0
  ↳ engine now requires core 1.1.0
```

## Identity verification

`basis verify` enforces that contributors use a company identity. For every repo
that has an e-mail-domain policy it checks:

* `git config user.email` resolves to an allowed domain, and
* the OpenPGP **signing key** (`user.signingkey`, or the key matching the git
  e-mail) has a user ID whose e-mail is on an allowed domain.

Domains come from `email_domain` (single) and/or `email_domains` (list). A repo
may override the constellation-wide policy with its own field. SSH-format signing
(`gpg.format=ssh`) carries no e-mail and is reported as unverifiable (not a
failure). The command exits non-zero if any checked repo fails.

```sh
$ basis verify
==> core
  allowed domains: corp.com
  ✓ git email: dev@corp.com
  ✓ gpg key: ABCD1234 [dev@corp.com]
  ✓ ok
```

`basis status` runs the same checks and shows a compact `id ✓ / id ✗ / id !`
column per repo (`—` when no policy applies), plus a summary line. Unlike
`basis verify`, `status` is informational and always exits 0; use `verify` as
the enforcing gate (e.g. in CI or a pre-push hook).

```sh
$ basis status
  core    rust  1.0.0       id ✓  main clean
  app     rust  1.0.0       id ✗  main dirty

versions: all versions at 1.0.0
identity: 1 repo(s) fail (run `basis verify` for details)
```

## Example

A runnable example lives in [`examples/`](examples/). From there:

```sh
basis -f examples/basis.yaml status
basis -f examples/basis.yaml version sync
basis -f examples/basis.yaml build -n
```
