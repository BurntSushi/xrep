require  'formula'
class Ripgrep < Formula
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  url "https://github.com/BurntSushi/ripgrep/archive/0.2.1.tar.gz"
  sha256 "372ccd0a93c98e9f3cc51644a9c52d1d8437ecb8b0e2908b33df9a46ca7b9ee2"

  depends_on "rust" => :build

  def install
    system "cargo", "build", "--release"
    bin.install "target/release/rg"
    man1.install "doc/rg.1"
  end

  def test
    system bin/"rg", "--help"
  end
end
