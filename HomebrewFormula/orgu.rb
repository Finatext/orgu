# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.1.4"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.4/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "bb707b7212d7ec4cbeec0b32d470a4f545a443313fefbe6f36f695bb4381d4e0"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.4/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "977a5bad477ab6ba529e8f74b03be0cc82c6111dba5d56ef1aca9b43c9ed06de"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
