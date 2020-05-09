Release Checklist
-----------------
* Run `cargo update` and review dependency updates. Commit updated
  `Cargo.lock`.
* Review changes for every crate in `crates` since the last ripgrep release.
  If the set of changes is non-empty, issue a new release for that crate.
* Edit the `Cargo.toml` to set the new ripgrep version. Run
  `cargo update -p ripgrep` so that the `Cargo.lock` is updated. Commit the
  changes.
* Create a new signed tag for the ripgrep release. Push it to GitHub.
* Wait for CI to finish creating the release. If the release build fails, then
  delete the tag from GitHub, make fixes, re-tag, delete the release and push.
* Copy the relevant section of the CHANGELOG to the tagged release notes.
* Run `ci/build-deb` locally and manually upload the deb package to the
  release.
* Run `cargo publish`.
* Run `ci/sha256-releases >> pkg/brew/ripgrep-bin.rb`. Then edit
  `pkg/brew/ripgrep-bin.rb` to update the version numbers and sha256 hashes.
  Remove extraneous stuff added by `ci/sha256-releases`. Commit changes.
