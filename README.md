# bfind
A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses Breadth-first search instead of Depth-first
search, written in [Rust](https://www.rust-lang.org/).

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
