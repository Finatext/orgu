# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.5.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.5.1/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "ab90c8fdddeeacd287a1e3b0a72094fa5150ccd6d4379bccbe32d61d844ebcb6"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.5.1/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "4ea494722500b6acafda8844c0a3aa667ac1cb7b86fa9835142c6438dcb63801"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
