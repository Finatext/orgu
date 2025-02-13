# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.2.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.2.1/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "50f1b336b3da5b08d0e2d43df72508f970454bea208a0216ef5e82c5b2c23053"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.2.1/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "d8979c8c25aaab6a59b4653c9fe84d4203a68351860323d69092fa904f89a5f2"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
