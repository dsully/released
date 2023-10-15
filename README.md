# Released

CLI for installing and updating GitHub released Artifacts.

## Installation

`libarchive` needs to be installed:

```shell
brew install libarchive
```

## Getting Started

```shell
released --help
```

## Config and State

Configuration is kept as a TOML file in `$XDG_CONFIG_HOME/released/config.toml`

What is actually installed and the version it is at is kept in a state file: `$XDG_STATE_HOME/released/installed.json`

### Inspired By

[gitrel](https://github.com/izirku/gitrel) and [vers](https://github.com/reynn/vers)
