# bfind
A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses Breadth-first search instead of Depth-first
search, written in [Rust](https://www.rust-lang.org/).

## Build

    $ cargo build

Or for the release version

    $ cargo build --release

## Install

    $ cargo install

## Usage

Currently, only basic directory listing is implemented.

List current working directory:

    $ bfind

List a specific directory:

    $ bfind /path/to/directory

## TODO

- Design a simple and powerful command line syntax.

## License

GNU LGPL v3
