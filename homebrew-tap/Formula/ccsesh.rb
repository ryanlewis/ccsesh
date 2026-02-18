class Ccsesh < Formula
  desc "List and resume recent Claude Code sessions"
  homepage "https://github.com/ryanlewis/ccsesh"
  version "0.1.0"
  license "MIT"

  on_macos do
    url "https://github.com/ryanlewis/ccsesh/releases/download/v#{version}/ccsesh-aarch64-apple-darwin.tar.gz"
    sha256 "PLACEHOLDER_SHA256_MACOS_ARM64"
  end

  on_linux do
    on_arm do
      url "https://github.com/ryanlewis/ccsesh/releases/download/v#{version}/ccsesh-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_ARM64"
    end

    on_intel do
      url "https://github.com/ryanlewis/ccsesh/releases/download/v#{version}/ccsesh-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "PLACEHOLDER_SHA256_LINUX_X86_64"
    end
  end

  def install
    bin.install "ccsesh"
  end

  test do
    assert_match "ccsesh", shell_output("#{bin}/ccsesh --help")
  end
end
