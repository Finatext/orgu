# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.1.2"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.2/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "c26af6cf3b8e73ed3a26e822da9cdb63ca7e24f9f01319466cc2f44089cdb27d"

      def install
        bin.install "orgu"
      end
    end

    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.2/orgu-x86_64-apple-darwin.tar.gz"
      sha256 "239314e0a6b041ae0e24bd6cdc0939ded67cb6a6f2d9550707385648b13773ba"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.2/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "2a37c0f932295e9c88dddea7148360d354828b0914b8fb61ba4f8a3bb38b986e"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
