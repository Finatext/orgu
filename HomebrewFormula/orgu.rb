# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.1.1"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.1/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "2b088b05af67cf334ba3279de6d37dfa5c1bc70175968a22d903061f780fc66d"

      def install
        bin.install "orgu"
      end
    end

    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.1/orgu-x86_64-apple-darwin.tar.gz"
      sha256 "360e42f90da281695ab8e9832c39e80a665dfcc8e0b2fff98cda9b8f7704a554"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.1.1/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "79bb153fa98999a57f7634d1b607f8922b36a96527abd062b59c2ac1cbc4c3de"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
