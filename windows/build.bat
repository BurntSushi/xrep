:: Automatically embed the manifest file on Windows MSVC.
:: Any arguments are passed through this script to `cargo rustc`.
@echo off
cargo rustc %* -- -C link-arg="/MANIFEST:EMBED" -C link-arg="/MANIFESTINPUT:%~dp0manifest.xml"
