# bfind
A [GNU find](https://www.gnu.org/software/findutils/)-like tool, but uses
Breadth First Search instead of Depth First Search, and is written in
[Rust](https://www.rust-lang.org/).

## Build

    $ cargo build

Or for the release version

    $ cargo build --release

## Install

    $ cargo install

## Roadmap

- Use two threads, one for listing files, one for printing.
- Be compliant with find's command line arguments.

## License

GNU LGPL v3
