# Development

These instructions are specific to Fedora; minor adjustments for your
platform may be required.

## Install host dependencies

```
sudo dnf install git openssh rustup
sudo dnf install @development-tools @development-libs
```

## Install rust and cargo

```
rustup-init ## just press enter when prompted for default selection
. "$HOME/.cargo/env"
```

## Clone source repository

```
git clone git@github.com:enigmacurry/openscad-part-maker.git \
  ~/git/vendor/enigmacurry/openscad-part-maker
cd ~/git/vendor/enigmacurry/openscad-part-maker
```

## Install development dependencies

```
cargo install just
just deps
```

## Build and run development openscad-part-maker

```
just run help
just run [ARGS ...]
```

## Build release binary

```
just build --release
```

## Create development alias

```
## Add this to ~/.bashrc or equivalent:
alias openscad-part-maker='just -f ~/git/vendor/enigmacurry/openscad-part-maker/Justfile run'
source <(openscad-part-maker completions bash 2> /dev/null)
```

Now you can run `openscad-part-maker` from any directory, with
any arguments, and it will automatically rebuild from source, and then
run it with those args. This will have full tab-completion in your shell.

## Testing

This project has incomplete testing. [See the latest coverage
report](https://enigmacurry.github.io/openscad-part-maker/coverage/master/).

## Run tests

```
# Run all tests:
just test

# Run a single test:
just test test_cli_help

# Verbose logging (which normally would be hidden for passing tests)
just test-verbose test_cli_help

# Auto run tests on source change:
just test-watch
```

## Clippy

```
just clippy
just clippy --fix
```

## Release (Github actions)

### Install cargo dependencies

```
just deps
```

### Bump release version and push new branch

The `bump-version` target will automatically update the version number
in Cargo.toml, Cargo.lock, and README.md as suggested by git-cliff.
This creates a new branch named `release-{VERSION}`, and automatically
checks it out. You just need to `git push` the branch:

```
just bump-version
# ... automatically checks out a new branch named release-{VERSION}

git push
```

### Make a new PR with the changeset

Branch protection is enabled, all changesets must come in the form of
a Pull Request. On GitHub, create a new Pull Request for the
`release-{VERSION}` branch into the master branch.

### Merge the PR and tag the release

Once the PR is merged, update your local repo, and run the release
target:

```
git checkout master
git pull
just release
```

New binaries will be automatically built by github actions, and a new
packaged release will be posted.
