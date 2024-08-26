# bfind

![build](https://github.com/brglng/bfind/actions/workflows/rust.yml/badge.svg)

A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses breadth-first search instead of depth-first search, written in [Rust](https://www.rust-lang.org/).

## Why

* BFS prefers files that are shallower, which means files in shallower directories are more likely to be found in a shorter time.
* When encountering a subdirectory which has many very deep subdirectories, BFS doesn't stuck on it before moving to the next subdirectory.
* I want to learn Rust by making this tool.

**NO WARRANTY:** I make this tool mainly for my personal use. I have no plan to improve its performance or features, neither are issues guaranteed to get fixed. However, PR is welcome.

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
$ cargo install --path .
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
$ bfind . -- name glob 'foo*' and type is dir
```

Print with formatting:

```sh
$ bfind . print 'file: {name:10}, {size:>10} bytes' -- name glob 'foo*' and size gt 1MiB
```

Execute a command:

```sh
$ bfind . exec cat '{fullpath}' -- name glob 'foo*.txt'
```

## TODO

- Design a simple and powerful command line syntax.
- Implement the command line interface.
