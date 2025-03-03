# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.4.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.4.1/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "a2f9ac69a9d38b14a97ad595f173832f4be06c0759af29a2ab199e3609ba7692"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.4.1/orgu-x86_64-unknown-linux-gnu.tar.gz"
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
