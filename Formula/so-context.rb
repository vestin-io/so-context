class SoContext < Formula
  desc "Context management layer for AI agents"
  homepage "https://github.com/vestin-io/so-context"
  license "MIT"
  head "https://github.com/vestin-io/so-context.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match "so-context", shell_output("#{bin}/so-context --help")
  end
end
