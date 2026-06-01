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

## Installing a constellation

`basis install` bootstraps a whole constellation from its manifest repository:

```sh
basis install acme/platform          # -> https://github.com/acme/platform
basis install git@github.com:acme/platform.git --into platform --branch main
```

It clones the manifest repo into a directory (named after the repo by default,
or `--into DIR`), reads its `basis.yaml`, and then clones every member repo with
a `url:` into its `path`, next to the manifest:

```
platform/
  basis.yaml          # from acme/platform
  core/               # cloned from its url:
  engine/             # cloned from its url:
```

`org/repo` is shorthand for a GitHub HTTPS URL; a full git URL (`https://…`,
`git@…`, `file://…`) works too. Members already present are skipped, members
without a `url:` are reported. After installing, run `basis status` /
`basis build` from inside the constellation directory.

## The manifest (`basis.yaml`)

```yaml
constellation: my-product
version: 1.2.0            # optional canonical version of the constellation
email_domain: corp.com   # optional identity policy (see `basis verify`)

repos:
  - name: core           # unique name, used with --repo
    path: core           # path relative to the manifest
    lang: rust           # rust | cpp
    url: https://github.com/acme/core   # optional canonical git URL
    actions:
      build: [cargo build --release]
      clean: [cargo clean]

  - name: engine
    path: engine
    lang: cpp
    url: git@github.com:acme/engine.git
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

# Any non-reserved name runs the matching `action` across the constellation:
basis <action> [--repo NAME]... [-k] [-n] [--tmux|--no-tmux]
basis build                              # run the `build` action everywhere
basis run                                # run the `run` action (e.g. services)
basis test --repo core --tmux            # run `test`, forced into a tmux display

# Reserved subcommands (cannot be used as action names):
basis install <org/repo> [--into DIR] [--branch B]   # clone a constellation
basis status                             # git + version status of all repos
basis verify                             # check git/GPG e-mail domains
basis display [NAME] [--detached|--kill] # launch a tmux dev dashboard

basis version                            # alias of `version show`
basis version show                       # list every repo's version
basis version set <X.Y.Z>                # set an explicit version everywhere
basis version sync [--to <X.Y.Z>]        # converge all repos onto one version
basis version bump <repo> [--major|--minor|--patch|--to X.Y.Z]
```

`build`, `clean`, `run`, `test`, … are not special — they are just action names
looked up in the manifest. The reserved names `install`, `status`, `verify`,
`display`, `version` are the only ones that cannot double as actions.

Common flags:

* `-f, --file <PATH>` — manifest path (default `basis.yaml`).
* `-r, --repo <NAME>` — restrict to specific repos (repeatable).
* `-k, --keep-going` — continue across repos even if one command fails.
* `-n, --dry-run` — print commands without executing them.
* `-t, --tmux` — run the action in a per-task tmux display (one pane per repo,
  in parallel); pairs with `--detached` and `--layout <L>`.

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

## tmux displays

### Per-task displays (driven by the manifest)

A task names a **display** in the manifest — the tmux session it runs in. When
that task is executed, basis spawns the display — one pane per repository, in
parallel — **lazily, at execution time** (nothing is created beforehand).

Displays are meant for **long-running tasks whose output you want to watch**
(running services, watchers). One-shot tasks like `build` or `clean` should stay
inline — just don't give them a `display`:

```yaml
tasks:
  run:
    display: services         # run this task in the "services" display
    layout: even-vertical
  # build / clean: no display — they run inline in the current terminal
```

With that, `basis run` creates a session named `services`, gives every selected
repo that defines the action its own pane (in the repo's directory, running the
action's commands), applies the layout and attaches. Repos without that action
are skipped. This is the "display под задачу" — declared once in the config,
born only when the task runs.

The natural use is a long-running `run` task — start every service and watch
its output live, each in its own pane:

```yaml
tasks:
  run:
    display: services
    layout: even-vertical     # stacked logs, one per repo
repos:
  - name: api
    actions: { run: ["cargo run --bin api"] }
  - name: worker
    actions: { run: ["cargo run --bin worker"] }
  - name: web
    actions: { run: ["npm run dev"] }
```

```sh
basis run            # api / worker / web each get a pane in "services", logs live
```

Commands are sent to a live shell, so a pane stays open after you Ctrl-C and you
can restart the process in place. Re-running `basis run` re-attaches to the same
session. Per-invocation overrides:

```sh
basis run                        # uses the task's display: setting
basis run --tmux                 # force a display (named <constellation>-run)
basis run --no-tmux              # force the current terminal for this run
basis run --detached             # with tmux: create but don't attach
basis run --layout tiled         # with tmux: override the layout
```

### Predefined dashboards (`displays:`)

A *display* can also be a named tmux session described in the manifest — a
standing dev dashboard (servers, watchers, logs, a scratch shell):

```yaml
displays:
  dev:
    session: myproj-dev      # optional, default <constellation>-<display>
    layout: tiled            # tiled | even-horizontal | even-vertical | main-vertical | ...
    panes:
      - { repo: core,   command: "cargo watch -x run" }   # cmd in the repo dir
      - { repo: engine, action: build }                   # reuse a repo action
      - { name: logs,   cwd: ., command: "tail -f log/dev.log" }
      - { name: shell }                                    # just a shell in base dir
```

```sh
basis display              # list configured displays
basis display dev          # create the session (if needed) and attach
basis display dev --detached   # create but don't attach (prints attach hint)
basis display dev --kill       # tear the session down
```

Each pane starts in `cwd` if given, else the `repo` directory, else the
manifest directory. Its command is `command` if given, else the named `action`
of `repo` (its commands joined with `&&`), else a plain shell. Commands are sent
to a live shell, so a pane stays open after its task exits and you can re-run it.
Re-running `basis display NAME` is idempotent — it attaches to the existing
session instead of recreating it.

`basis status` lists every configured display and whether its tmux session is
currently up:

```
displays:
  dev      ● running  3 pane(s), tiled            [demo-dev]
  tests    ○ stopped  2 pane(s), even-horizontal  [demo-tests]
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
  core    rust  1.0.0       id ✓  main clean origin✓
  app     rust  1.0.0       id ✗  main dirty origin✗

versions: all versions at 1.0.0
identity: 1 repo(s) fail (run `basis verify` for details)
```

## Canonical repository URLs

Each repo may declare a canonical git `url:`. `basis status` compares it against
the local `origin` remote and reports one of:

* `origin✓` — `origin` matches the canonical URL,
* `origin✗` — `origin` points somewhere else (the expected/actual pair is
  listed below the table),
* `no-origin` — the repo has no `origin` remote,
* `missing` — the repo directory has not been cloned yet.

URLs are compared after normalisation, so `git@github.com:acme/core.git` and
`https://github.com/acme/core` are treated as the same repository (scheme,
`git@` userinfo, and a trailing `.git` are ignored).

## Example

A runnable example lives in [`examples/`](examples/). From there:

```sh
basis -f examples/basis.yaml status
basis -f examples/basis.yaml version sync
basis -f examples/basis.yaml build -n
```
