# Contributing to rmpc

Thank you for taking interest in helping rmpc! Hopefully this short guide will help you run and 
debug the project.

## Table of contents

* [Prerequisites](#prerequisites)
* [Dev setup and running rmpc from the source code](#dev-setup-and-running-rmpc-from-the-source-code)
* [Formatting the code](#formatting-the-code)
* [Reporting an issue](#reporting-an-issue)
* [Requesting a feature](#requesting-a-feature)
* [Submitting a pull request](#submitting-a-pull-request)
* [Documentation](#documentation)

## Prerequisites

* This guide assumes you are reasonably familiar with git and github.
* You will also need to have rust installed on your system. You can install it from the official 
website https://www.rust-lang.org/tools/install

## Dev setup and running rmpc from the source code

Running rmpc is very straight forward.

1. Ensure you have latest version of rust installed.

2. Clone the repository and switch to the newly created directory
```bash
git clone https://github.com/mierak/rmpc.git
cd rmpc
```

3. Run rmpc 
```bash
# in debug mode
cargo run
# or in release mode
cargo run --release
```

4. Setup your config file for debug mode

Rmpc will search for `config.debug.ron` instead of `config.ron` in debug mode. This means you do 
not have to change your usual config file to develop and debug rmpc.
You can also use a special `Logs` pane your debug config which displays logs directly in rmpc.

## Formatting the code

Rmpc uses `rustfmt` nightly to format the code.
1. Install nightly `rustfmt`
```bash
rustup component add rustfmt --toolchain nightly
```
2. Format the code
```bash
cargo +nightly fmt --all
```

## Reporting an issue

Please fill the [Bug Report](https://github.com/mierak/rmpc/issues/new?template=bug.yml) template
if you encounter an issue which is not yet solved in the current git version of rmpc and include
all information relevant to your issue.

Sometimes you might be asked to provide `trace` level logs. You can obtain them by running rmpc
with `RUST_LOG` environment variable set to trace. The log file is located at `/tmp/rmpc_${UID}.log`.
These logs can be very verbose so you might have to upload them somewhere and link them to the issue.
```bash
RUST_LOG=trace rmpc
# or if you are running directly from source code
RUST_LOG=trace cargo run
```

## Requesting a feature

Similar to the bug report, please fill out the [Feature Request](https://github.com/mierak/rmpc/issues/new?template=feature.yml)
template.

## Submitting a pull request

This is just a small checklist to potentially reduce the amount of back and forth when submitting
pull requests.

* [Format](#formatting-the-code) the code
* Run clippy (stable)
* Ensure that all tests pass by running `cargo test`
* Update the documentation if your feature changed or added behavior
* Note your change in the changelog 
* Address all comments in the pull request

## Documentation

The documentation lies in the `docs` directory in the repository root. The docs area created with
[astro](https://astro.build/) javascript framework. All contributions and improvements to the
documentation are welcome.

There are multiple sections in the docs, primarily `src/content/docs/next` and `src/content/docs/release`
which correspond to the current dev and release version respectively.

To run the documentation locally:

1. Ensure you have [node.js](https://nodejs.org/en) installed.
2. Go to the docs directory and install dependencies
```bash
cd docs
npm install
```
3. Run the dev server
```bash
npm run dev
```

A local HTTP server will be started. Navigate to http://localhost:4321/rmpc and you should see the
docs website. All changes you make to the docs should be automatically reflected in your browser.

