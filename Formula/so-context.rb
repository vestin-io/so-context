class SoContext < Formula
  desc "Context management layer for AI agents"
  homepage "https://github.com/vestin-io/so-context"
  license "MIT"

  stable do
    url "git@github.com:vestin-io/so-context.git",
        using: GitDownloadStrategy,
        tag: "v0.0.1",
        revision: "c12834b5f68d94be79851c6eda18a639a5ec7e96"
  end

  head "git@github.com:vestin-io/so-context.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: ".")
  end

  test do
    assert_match "so-context", shell_output("#{bin}/so-context --help")
  end
end
