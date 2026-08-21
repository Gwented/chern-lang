# README.md

## chrn_core/
Core chrn language library

## chrn_tools/
chrn related tooling

## common/
Contains general purpose tooling any other crate would use like algorithms, terminal color settings, etsy.

Is a workspace as opposed to a single `common` crate so that parts of the workspace can be used in isolation instead of needing to depend on the entirety of common.
