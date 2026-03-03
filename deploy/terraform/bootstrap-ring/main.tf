locals {
  user_data = <<-EOT
    #!/bin/bash
    set -eux
    dnf update -y
    dnf install -y docker
    systemctl enable docker
    systemctl start docker
    docker run -d --network host \
      ghcr.io/trentpierce/shard-daemon:${var.shard_version} \
      --node-role bootstrap --tcp-port 4001 --control-port 9091
  EOT
}

data "aws_ami" "al2023_use1" {
  provider    = aws.use1
  most_recent = true
  owners      = ["137112412989"]

  filter {
    name   = "name"
    values = ["al2023-ami-2023*-x86_64"]
  }
}

data "aws_ami" "al2023_euw1" {
  provider    = aws.euw1
  most_recent = true
  owners      = ["137112412989"]

  filter {
    name   = "name"
    values = ["al2023-ami-2023*-x86_64"]
  }
}

data "aws_ami" "al2023_apse1" {
  provider    = aws.apse1
  most_recent = true
  owners      = ["137112412989"]

  filter {
    name   = "name"
    values = ["al2023-ami-2023*-x86_64"]
  }
}

data "aws_ami" "al2023_usw2" {
  provider    = aws.usw2
  most_recent = true
  owners      = ["137112412989"]

  filter {
    name   = "name"
    values = ["al2023-ami-2023*-x86_64"]
  }
}

data "aws_ami" "al2023_sae1" {
  provider    = aws.sae1
  most_recent = true
  owners      = ["137112412989"]

  filter {
    name   = "name"
    values = ["al2023-ami-2023*-x86_64"]
  }
}

resource "aws_security_group" "bootstrap_use1" {
  provider = aws.use1
  name     = "shard-bootstrap-us-east-1"

  ingress { from_port = 4001 to_port = 4001 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9092 to_port = 9092 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9091 to_port = 9091 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  egress  { from_port = 0 to_port = 0 protocol = "-1" cidr_blocks = ["0.0.0.0/0"] }
}

resource "aws_security_group" "bootstrap_euw1" {
  provider = aws.euw1
  name     = "shard-bootstrap-eu-west-1"

  ingress { from_port = 4001 to_port = 4001 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9092 to_port = 9092 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9091 to_port = 9091 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  egress  { from_port = 0 to_port = 0 protocol = "-1" cidr_blocks = ["0.0.0.0/0"] }
}

resource "aws_security_group" "bootstrap_apse1" {
  provider = aws.apse1
  name     = "shard-bootstrap-ap-southeast-1"

  ingress { from_port = 4001 to_port = 4001 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9092 to_port = 9092 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9091 to_port = 9091 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  egress  { from_port = 0 to_port = 0 protocol = "-1" cidr_blocks = ["0.0.0.0/0"] }
}

resource "aws_security_group" "bootstrap_usw2" {
  provider = aws.usw2
  name     = "shard-bootstrap-us-west-2"

  ingress { from_port = 4001 to_port = 4001 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9092 to_port = 9092 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9091 to_port = 9091 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  egress  { from_port = 0 to_port = 0 protocol = "-1" cidr_blocks = ["0.0.0.0/0"] }
}

resource "aws_security_group" "bootstrap_sae1" {
  provider = aws.sae1
  name     = "shard-bootstrap-sa-east-1"

  ingress { from_port = 4001 to_port = 4001 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9090 to_port = 9090 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9092 to_port = 9092 protocol = "udp" cidr_blocks = ["0.0.0.0/0"] }
  ingress { from_port = 9091 to_port = 9091 protocol = "tcp" cidr_blocks = ["0.0.0.0/0"] }
  egress  { from_port = 0 to_port = 0 protocol = "-1" cidr_blocks = ["0.0.0.0/0"] }
}

resource "aws_instance" "bootstrap_use1" {
  provider          = aws.use1
  ami               = data.aws_ami.al2023_use1.id
  instance_type     = var.instance_type
  user_data         = local.user_data
  vpc_security_group_ids = [aws_security_group.bootstrap_use1.id]

  tags = { Name = "shard-bootstrap-us-east-1", Role = "bootstrap" }
}

resource "aws_instance" "bootstrap_euw1" {
  provider          = aws.euw1
  ami               = data.aws_ami.al2023_euw1.id
  instance_type     = var.instance_type
  user_data         = local.user_data
  vpc_security_group_ids = [aws_security_group.bootstrap_euw1.id]

  tags = { Name = "shard-bootstrap-eu-west-1", Role = "bootstrap" }
}

resource "aws_instance" "bootstrap_apse1" {
  provider          = aws.apse1
  ami               = data.aws_ami.al2023_apse1.id
  instance_type     = var.instance_type
  user_data         = local.user_data
  vpc_security_group_ids = [aws_security_group.bootstrap_apse1.id]

  tags = { Name = "shard-bootstrap-ap-southeast-1", Role = "bootstrap" }
}

resource "aws_instance" "bootstrap_usw2" {
  provider          = aws.usw2
  ami               = data.aws_ami.al2023_usw2.id
  instance_type     = var.instance_type
  user_data         = local.user_data
  vpc_security_group_ids = [aws_security_group.bootstrap_usw2.id]

  tags = { Name = "shard-bootstrap-us-west-2", Role = "bootstrap" }
}

resource "aws_instance" "bootstrap_sae1" {
  provider          = aws.sae1
  ami               = data.aws_ami.al2023_sae1.id
  instance_type     = var.instance_type
  user_data         = local.user_data
  vpc_security_group_ids = [aws_security_group.bootstrap_sae1.id]

  tags = { Name = "shard-bootstrap-sa-east-1", Role = "bootstrap" }
}
