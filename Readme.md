shellexpand, a library for shell-like expansion in strings
==========================================================

[![Build Status][travis]](https://travis-ci.org/netvl/shellexpand) [![crates.io][crates]](https://crates.io/crates/shellexpand)

  [travis]: https://img.shields.io/travis/netvl/shellexpand.svg?style=flat-square
  [crates]: https://img.shields.io/crates/v/shellexpand.svg?style=flat-square

[Documentation](https://docs.rs/shellexpand/)

shellexpand is a single dependency library which allows one to perform shell-like expansions in strings,
that is, to expand variables like `$A` or `${B}` into their values inside some context and to expand
`~` in the beginning of a string into the home directory (again, inside some context).

This crate provides generic functions which accept arbitrary contexts as well as default, system-based
functions which perform expansions using the system-wide context (represented by functions from `std::env`
module and [dirs](https://crates.io/crates/dirs) crate).

## Usage

Just add a dependency in your `Cargo.toml`:

```toml
[dependencies]
shellexpand = "1.1.1"
```

See the crate documentation (a link is present in the beginning of this readme) for more information
and examples.


## Changelog

### Version 1.1.1

* Bump `dirs` dependency to 2.0.

### Version 1.1.0

* Changed use of deprecated `std::env::home_dir` to the [dirs](https://crates.io/crates/dirs)::home_dir function

### Version 1.0.0

* Fixed typos and minor incompletenesses in the documentation
* Changed `home_dir` argument type for tilde expansion functions to `FnOnce` instead `FnMut`
* Changed `LookupError::name` field name to `var_name`

### Version 0.1.0

* Initial release.

## License

This program is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed 
as above, without any additional terms or conditions.
