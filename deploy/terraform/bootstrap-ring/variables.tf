variable "aws_regions" {
  description = "Expected bootstrap ring regions."
  type        = list(string)
  default     = ["us-east-1", "eu-west-1", "ap-southeast-1", "us-west-2", "sa-east-1"]
}

variable "instance_type" {
  description = "EC2 instance type for bootstrap nodes."
  type        = string
  default     = "t3.small"
}

variable "shard_version" {
  description = "Container tag for shard-daemon image."
  type        = string
  default     = "latest"
}
