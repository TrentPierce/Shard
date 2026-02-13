/**
 * Shard Node Discovery Client
 * 
 * Fetches available Shard nodes from multiple sources:
 * 1. Local Rust daemon (/v1/system/topology)
 * 2. Community list (GitHub Pages JSON)
 * 3. DNS TXT records (shard.network)
 * 4. IPFS content routing
 */

export interface ShardNode {
  peer_id: string
  api_url: string
  latency_ms?: number
  capacity?: number
}

interface DiscoveryResponse {
  nodes: ShardNode[]
  source: 'local' | 'community' | 'dns' | 'ipfs' | 'merged'
  last_updated: string
}

/**
 * Get Shard nodes from local daemon
 */
export async function getLocalShardNodes(): Promise<DiscoveryResponse> {
  try {
    const response = await fetch('/v1/system/topology')
    if (!response.ok) throw new Error('Local topology fetch failed')
    
    const topology = await response.json()
    
    if (topology.status !== 'ok') {
      return {
        nodes: [],
        source: 'local',
        last_updated: new Date().toISOString(),
      }
    }
    
    // Return local node if it's serving as public API
    const nodes: ShardNode[] = []
    if (topology.public_api && topology.public_api_addr) {
      nodes.push({
        peer_id: topology.shard_peer_id,
        api_url: `http://${topology.public_api_addr}`,
      })
    }
    
    return {
      nodes,
      source: 'local',
      last_updated: new Date().toISOString(),
    }
  } catch (error) {
    console.error('[Discovery] Local daemon error:', error)
    return {
      nodes: [],
      source: 'local',
      last_updated: new Date().toISOString(),
    }
  }
}

/**
 * Get Shard nodes from community GitHub list
 */
export async function getCommunityShardNodes(): Promise<DiscoveryResponse> {
  try {
    const response = await fetch('https://raw.githubusercontent.com/ShardNetwork/nodes/main.json', {
      cache: 'reload',
    })
    
    if (!response.ok) throw new Error('Community nodes fetch failed')
    
    const nodes = await response.json()
    
    return {
      nodes: nodes.shard_nodes || [],
      source: 'community',
      last_updated: nodes.last_updated || new Date().toISOString(),
    }
  } catch (error) {
    console.error('[Discovery] Community fetch error:', error)
    return {
      nodes: [],
      source: 'community',
      last_updated: new Date().toISOString(),
    }
  }
}

/**
 * Get Shard nodes from IPFS (via DNSLink or gateway)
 */
export async function getIPFSShardNodes(): Promise<DiscoveryResponse> {
  try {
    // Try IPFS gateway first
    const response = await fetch('https://shard.network/api/nodes.json', {
      cache: 'reload',
    })
    
    if (!response.ok) throw new Error('IPFS fetch failed')
    
    const data = await response.json()
    
    return {
      nodes: data.shard_nodes || [],
      source: 'ipfs',
      last_updated: data.last_updated || new Date().toISOString(),
    }
  } catch (error) {
    console.error('[Discovery] IPFS fetch error:', error)
    return {
      nodes: [],
      source: 'ipfs',
      last_updated: new Date().toISOString(),
    }
  }
}

/**
 * Get all Shard nodes from all sources (merged)
 */
export async function getAllShardNodes(): Promise<DiscoveryResponse> {
  const [local, community, ipfs] = await Promise.all([
    getLocalShardNodes(),
    getCommunityShardNodes(),
    getIPFSShardNodes(),
  ])
  
  // Merge all nodes, deduplicate by peer_id
  const allNodes = new Map<string, ShardNode>()
  
  for (const response of [local, community, ipfs]) {
    for (const node of response.nodes) {
      const existing = allNodes.get(node.peer_id)
      if (!existing || node.latency_ms !== undefined) {
        allNodes.set(node.peer_id, node)
      }
    }
  }
  
  const mergedNodes = Array.from(allNodes.values())
  
  return {
    nodes: mergedNodes,
    source: 'merged',
    last_updated: new Date().toISOString(),
  }
}

/**
 * Select fastest Shard node based on latency
 */
export function selectFastestShard(nodes: ShardNode[]): ShardNode | null {
  if (nodes.length === 0) return null
  
  // Filter nodes with known latency
  const nodesWithLatency = nodes.filter(n => n.latency_ms !== undefined)
  
  if (nodesWithLatency.length === 0) {
    // No latency data, return random node
    return nodes[Math.floor(Math.random() * nodes.length)]
  }
  
  // Sort by latency (ascending)
  nodesWithLatency.sort((a, b) => (a.latency_ms! - b.latency_ms!))
  
  return nodesWithLatency[0]
}

/**
 * Get the best Shard API URL for making requests.
 * Falls back to configured API_BASE if no shard nodes are available.
 */
export async function getBestShardApiUrl(): Promise<string> {
  // Import config here to avoid circular dependencies
  const { apiUrl: configApiUrl } = await import("./config")
  
  try {
    // Try to get shard nodes from discovery
    const discovery = await getAllShardNodes()
    
    if (discovery.nodes.length > 0) {
      const fastest = selectFastestShard(discovery.nodes)
      if (fastest && fastest.api_url) {
        return fastest.api_url
      }
    }
  } catch (error) {
    console.warn("[Discovery] Failed to get shard nodes, using config URL:", error)
  }
  
  // Fallback to configured API URL
  return configApiUrl("/v1")
}
