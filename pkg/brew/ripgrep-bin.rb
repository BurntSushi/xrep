class RipgrepBin < Formula
  version '0.7.1'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if OS.mac?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "ee670b0fba46323ee9a2d1c5b8bee46fa3e45778f6f105f2e8e9ee29e8bd0d45"
  elsif OS.linux?
      url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-unknown-linux-musl.tar.gz"
      sha256 "ac595c2239b9a30e0e0744578afa6b73e32cdd8ae61d4f1c0ee5d6b55adbadcf"
  end

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"

    bash_completion.install "complete/rg.bash-completion"
    fish_completion.install "complete/rg.fish"
    zsh_completion.install "complete/_rg"
  end
end
