# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.3.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.3.0/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "1a7b27596155234acae7a08c9086f97f500ef17fce9dd5b898e53ab422f6913f"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.3.0/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "83b93b0197189cd652d970e0018ea43e36ca8b8009b2b46e2d9e897fb6352e7a"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
