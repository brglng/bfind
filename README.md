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

## TODO

- Design a simple and powerful command line syntax.

## License

GNU LGPL v3
