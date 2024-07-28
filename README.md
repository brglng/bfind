# bfind

![build](https://github.com/brglng/bfind/actions/workflows/rust.yml/badge.svg)

A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses breadth-first traversal instead of depth-first traversal, written in [Rust](https://www.rust-lang.org/).

**NO WARRANTY:** This is only a tool for my personal use. I have no plan to improve its performance or features, neither are issues guaranteed to get fixed, but PR is welcome.

## Build

```sh
$ cargo build
```

Or for the release version

```sh
$ cargo build --release
```

## Install

```sh
$ cargo install
```

## Usage

Currently, only basic directory listing is implemented.

List current working directory:

```sh
$ bfind
```

List a specific directory:

```sh
$ bfind /path/to/directory
```

Find a file with regular expression:

```sh
$ bfind . -- name match 'foo.*'
```

Find a file with glob:

```sh
$ bfind . -- name glob 'foo*'
```

Combining conditions:

```sh
$ bfind . -- name glob 'foo*' and type == dir
```

Print with formatting:

```sh
$ bfind . print 'file: {name:10}, {size:>10} bytes' -- name glob 'foo*' and size gt 1MiB
```

Execute a command:

```sh
$ bfind . exec cat '{name}' -- name glob 'foo*.txt'
```

## TODO

- Design a simple and powerful command line syntax.
- Implement the command line interface.

## License

GNU LGPL v3
