class Composit < Formula
  desc "Governance-as-Code for AI-generated infrastructure"
  homepage "https://nuetzliches.github.io/composit"
  license "MIT"
  version "0.5.0"

  on_macos do
    on_arm do
      url "https://github.com/nuetzliches/composit/releases/download/v#{version}/composit-aarch64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_APPLE"
    end
    on_intel do
      url "https://github.com/nuetzliches/composit/releases/download/v#{version}/composit-x86_64-apple-darwin.tar.gz"
      sha256 "PLACEHOLDER_X86_64_APPLE"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/nuetzliches/composit/releases/download/v#{version}/composit-aarch64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_AARCH64_LINUX"
    end
    on_intel do
      url "https://github.com/nuetzliches/composit/releases/download/v#{version}/composit-x86_64-unknown-linux-musl.tar.gz"
      sha256 "PLACEHOLDER_X86_64_LINUX"
    end
  end

  def install
    bin.install "composit"
  end

  test do
    assert_match "composit", shell_output("#{bin}/composit --version")
  end
end
