# bfind
A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses
Breadth First Search instead of Depth First Search, written in
[Rust](https://www.rust-lang.org/).

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

## Roadmap

- Use three threads, one for listing files, one for filtering and,
  one for printing
- Be compliant with `find`'s command line arguments

## License

GNU LGPL v3
