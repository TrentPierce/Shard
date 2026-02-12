class Shard < Formula
  desc "Shard Oracle - P2P distributed inference daemon"
  homepage "https://shard.network"
  url "https://github.com/TrentPierce/Shard/releases/download/v0.4.0/shard-0.4.0-linux-x86_64.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256"
  version "0.4.0"
  license "MIT"

  depends_on "openssl@3.0"

  def install
    bin.install "shard-daemon"
  end

  plist_options :startup => true

  def plist
    <<~EOS
      <?xml version="1.0" encoding="UTF-8"?>
      <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
      <plist version="1.0">
      <dict>
        <key>Label</key>
        <string>network.shard.daemon</string>
        <key>ProgramArguments</key>
        <array>
          <string>#{opt_bin}/shard-daemon</string>
          <string>--contribute</string>
        </array>
        <key>RunAtLoad</key>
        <true/>
        <key>KeepAlive</key>
        <true/>
        <key>StandardOutPath</key>
        <string>#{var}/log/shard.log</string>
        <key>StandardErrorPath</key>
        <string>#{var}/log/shard.error.log</string>
      </dict>
      </plist>
    EOS
  end

  test do
    system "#{bin}/shard-daemon", "--version"
  end
end
