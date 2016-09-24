require  'formula'
class Ripgrep < Formula
  version '0.1.17'
  desc "Search tool like grep and The Silver Searcher."
  homepage "https://github.com/BurntSushi/ripgrep"

  if Hardware::CPU.is_64_bit?
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-x86_64-apple-darwin.tar.gz"
    sha256 "fc138cd57b533bd65739f3f695322e483fe648736358d853ddb9bcd26d84fdc5"
  else
    url "https://github.com/BurntSushi/ripgrep/releases/download/#{version}/ripgrep-#{version}-i686-apple-darwin.tar.gz"
    sha256 "3ce1f12e49a463bc9dd4cfe2537aa9989a0dc81f7aa6f959ee0d0d82b5f768cb"
  end

  def install
    bin.install "rg"
    man1.install "rg.1"
  end
end
