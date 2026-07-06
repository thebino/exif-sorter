# Homebrew formula template for a binary release.
#
# Setup (one-time):
#   1. Create a repository named `homebrew-tap` under the thebino account.
#   2. Copy this file into it as Formula/exif-sorter.rb.
#   3. After each release, update `version` and the two sha256 values from
#      the *.sha256 assets attached to the GitHub release.
#
# Users then install with:
#   brew tap thebino/tap
#   brew install exif-sorter
class ExifSorter < Formula
  desc "Sort images into date-based folders using their EXIF data"
  homepage "https://github.com/thebino/exif-sorter"
  version "1.0.0"
  license "AGPL-3.0-only"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/thebino/exif-sorter/releases/download/v#{version}/exif-sorter-Darwin-aarch64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ASSET"
    else
      url "https://github.com/thebino/exif-sorter/releases/download/v#{version}/exif-sorter-Darwin-x86_64.tar.gz"
      sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ASSET"
    end
  end

  on_linux do
    url "https://github.com/thebino/exif-sorter/releases/download/v#{version}/exif-sorter-Linux-x86_64-musl.tar.gz"
    sha256 "REPLACE_WITH_SHA256_FROM_RELEASE_ASSET"
  end

  def install
    bin.install "exif-sorter"
    man1.install "exif-sorter.1"
    bash_completion.install "completions/exif-sorter.bash" => "exif-sorter"
    zsh_completion.install "completions/exif-sorter.zsh" => "_exif-sorter"
    fish_completion.install "completions/exif-sorter.fish"
  end

  test do
    assert_match "exif-sorter", shell_output("#{bin}/exif-sorter --help")
  end
end
