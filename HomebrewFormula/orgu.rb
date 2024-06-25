# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.1.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.0/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "a87714a3406544244e427b584bfddd430e372846ad7ede8b71314552be349534"

      def install
        bin.install "orgu"
      end
    end

    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.0/orgu-x86_64-apple-darwin.tar.gz"
      sha256 "7135a2c8bf04df271310adf8b05a826de37a2a8369f04fc1f377d64368646c49"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.0/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "27b86e82e47b849ddc3c3d4e2ae9444545026df3fe59ccd5ac21f33ef0992304"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
