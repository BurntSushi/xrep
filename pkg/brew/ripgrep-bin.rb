class RipgrepBin < Formula
  version '11.0.2'
  desc "Recursively search directories for a regex pattern."
  homepage "https://github.com/BurntSushi/ripgrep"

  if OS.mac?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0ba26423691deedf2649b12b1abe3d2be294ee1cb17c40b68fe85efe194f4f57"
  elsif OS.linux?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "2e7978e346553fbc45c0940d9fa11e12f9afbae8213b261aad19b698150e169a"
  end

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "doc/rg.1"

    bash_completion.install "complete/rg.bash"
    fish_completion.install "complete/rg.fish"
    zsh_completion.install "complete/_rg"
  end
end
