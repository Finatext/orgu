# typed: false
# frozen_string_literal: true

class Orgu < Formula
  desc "orgu is a tool for implementing organization-wide workflows on GitHub"
  homepage "https://github.com/Finatext/orgu"
  version "0.2.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/Finatext/orgu/releases/download/v0.2.0/orgu-aarch64-apple-darwin.tar.gz"
      sha256 "17636931fe666414370f939a5bdcc762825149190336a9bdbc94ccefdba22805"

      def install
        bin.install "orgu"
      end
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/Finatext/orgu/releases/download/v0.2.0/orgu-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "634a342bcdb858b6d7913c8e26a00afa4a68d918af981a5b9a3ffb1f2986b0a8"

      def install
        bin.install "orgu"
      end
    end
  end

  test do
    system "#{bin}/orgu --version"
  end
end
