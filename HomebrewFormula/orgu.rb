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
      sha256 "0816e093f2494c6baae06f3a04a177dd325abf7d00ecf7811eb1e7dd6a11faaf"

      def install
        bin.install "orgu"
      end
    end

    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.0/orgu-x86_64-apple-darwin.tar.gz"
      sha256 "5a8f680a52536cdb96c28c42b08bc7c5c05a48067b3066498dfc1d23a1ed4483"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.0/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "a8f4c0776441145784cf16d3d8975c4e7a57413a3976249294fb01f1e975fecc"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
