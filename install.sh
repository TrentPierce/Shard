#!/bin/bash
# curl -sSL https://shard.network/install.sh | bash
VERSION=$(curl -s https://api.github.com/repos/TrentPierce/Shard/releases/latest | grep tag_name | cut -d'"' -f4)
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
curl -L "https://github.com/TrentPierce/Shard/releases/download/${VERSION}/shard-daemon-${OS}-${ARCH}.tar.gz" | tar xz
sudo mv shard-daemon /usr/local/bin/
shard-daemon --contribute
