# openscad-part-maker

This is a self-service web frontend for making custom 3d printed parts
via an OpenSCAD template. It presents a web form for a user to upload
SVG assets and to specify custom parameters. It has an API for
processing these inputs and downloading to the user's browser the
resulting .STL file.

## Install

[Download the latest release for your
platform.](https://github.com/enigmacurry/openscad-part-maker/releases)

### Tab completion

To install tab completion support, put this in your `~/.bashrc` (assuming you use Bash):

```
### Bash completion for openscad-part-maker (Put this in ~/.bashrc)
source <(openscad-part-maker completions bash)
```

If you don't like to type out the full name `openscad-part-maker`, you can make
a shorter alias (`h`), as well as enable tab completion for the alias
(`h`):

```
### Alias openscad-part-maker as h (Put this in ~/.bashrc):
alias h=openscad-part-maker
complete -F _openscad-part-maker -o bashdefault -o default h
```

Completion for Zsh and/or Fish has also been implemented, but the
author has not tested this:

```
### Zsh completion for openscad-part-maker (Put this in ~/.zshrc):
autoload -U compinit; compinit; source <(openscad-part-maker completions zsh)

### Fish completion for openscad-part-maker (Put this in ~/.config/fish/config.fish):
openscad-part-maker completions fish | source
```

## Usage

```
$ openscad-part-maker

Usage: openscad-part-maker [OPTIONS] [COMMAND]

Commands:

Options:
  -h, --help                  Print help
  -V, --version               Print version
```

## Development

See [DEVELOPMENT.md](DEVELOPMENT.md)
