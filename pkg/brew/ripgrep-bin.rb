class RipgrepBin < Formula
  version '0.4.0'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"
  url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
  sha256 "6ac71251909227f8ef7eda27d3080c954843f3665b81e455362c90b2a9c4734a"

  conflicts_with "ripgrep"

  def install
    bin.install "rg"
    man1.install "rg.1"
    
    bash_completion.install "completion/rg.bash-completion"
    fish_completion.install "completion/rg.fish"
    zsh_completion.install "completion/_rg"
  end
end
