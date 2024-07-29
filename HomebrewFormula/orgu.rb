# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.1.3"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.3/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "f8a31ee34063750699d288271f0ff946d7541d5f546f0692b1d68f6f636f8b77"

      def install
        bin.install "orgu"
      end
    end

    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.3/orgu-x86_64-apple-darwin.tar.gz"
      sha256 "15ef10b382e202541194f17b95d382be0aa1a47570116d9e7c1f3337c57baf8f"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.3/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "8c107783af57ed688eeb67c98dbbe564a84a8b3f385616803f84281450728c43"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
