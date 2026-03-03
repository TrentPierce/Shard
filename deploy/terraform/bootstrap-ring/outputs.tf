output "bootstrap_peer_addresses" {
  description = "Formatted libp2p multiaddrs for provisioned bootstrap nodes."
  value = [
    "/ip4/${aws_instance.bootstrap_use1.public_ip}/tcp/4001/p2p/REPLACE_US_EAST_PEER_ID",
    "/ip4/${aws_instance.bootstrap_euw1.public_ip}/tcp/4001/p2p/REPLACE_EU_WEST_PEER_ID",
    "/ip4/${aws_instance.bootstrap_apse1.public_ip}/tcp/4001/p2p/REPLACE_AP_SE_PEER_ID",
    "/ip4/${aws_instance.bootstrap_usw2.public_ip}/tcp/4001/p2p/REPLACE_US_WEST_PEER_ID",
    "/ip4/${aws_instance.bootstrap_sae1.public_ip}/tcp/4001/p2p/REPLACE_SA_EAST_PEER_ID",
  ]
}
