/**
 * Oracle Node Discovery Client
 * 
 * Fetches available Oracle nodes from multiple sources:
 * 1. Local Rust daemon (/v1/system/topology)
 * 2. Community list (GitHub Pages JSON)
 * 3. DNS TXT records (shard.network)
 * 4. IPFS content routing
 */

export interface OracleNode {
  peer_id: string
  api_url: string
  latency_ms?: number
  capacity?: number
}

interface DiscoveryResponse {
  nodes: OracleNode[]
  source: 'local' | 'community' | 'dns' | 'ipfs' | 'merged'
  last_updated: string
}

/**
 * Get Oracle nodes from local daemon
 */
export async function getLocalOracleNodes(): Promise<DiscoveryResponse> {
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
    const nodes: OracleNode[] = []
    if (topology.public_api && topology.public_api_addr) {
      nodes.push({
        peer_id: topology.oracle_peer_id,
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
 * Get Oracle nodes from community GitHub list
 */
export async function getCommunityOracleNodes(): Promise<DiscoveryResponse> {
  try {
    const response = await fetch('https://raw.githubusercontent.com/ShardNetwork/nodes/main.json', {
      cache: 'reload',
    })
    
    if (!response.ok) throw new Error('Community nodes fetch failed')
    
    const nodes = await response.json()
    
    return {
      nodes: nodes.oracle_nodes || [],
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
 * Get Oracle nodes from IPFS (via DNSLink or gateway)
 */
export async function getIPFSOracleNodes(): Promise<DiscoveryResponse> {
  try {
    // Try IPFS gateway first
    const response = await fetch('https://shard.network/api/nodes.json', {
      cache: 'reload',
    })
    
    if (!response.ok) throw new Error('IPFS fetch failed')
    
    const data = await response.json()
    
    return {
      nodes: data.oracle_nodes || [],
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
 * Get all Oracle nodes from all sources (merged)
 */
export async function getAllOracleNodes(): Promise<DiscoveryResponse> {
  const [local, community, ipfs] = await Promise.all([
    getLocalOracleNodes(),
    getCommunityOracleNodes(),
    getIPFSOracleNodes(),
  ])
  
  // Merge all nodes, deduplicate by peer_id
  const allNodes = new Map<string, OracleNode>()
  
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
 * Select fastest Oracle node based on latency
 */
export function selectFastestOracle(nodes: OracleNode[]): OracleNode | null {
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
 * Get the best Oracle API URL for making requests.
 * Falls back to configured API_BASE if no oracle nodes are available.
 */
export async function getBestOracleApiUrl(): Promise<string> {
  // Import config here to avoid circular dependencies
  const { apiUrl: configApiUrl } = await import("./config")
  
  try {
    // Try to get oracle nodes from discovery
    const discovery = await getAllOracleNodes()
    
    if (discovery.nodes.length > 0) {
      const fastest = selectFastestOracle(discovery.nodes)
      if (fastest && fastest.api_url) {
        return fastest.api_url
      }
    }
  } catch (error) {
    console.warn("[Discovery] Failed to get oracle nodes, using config URL:", error)
  }
  
  // Fallback to configured API URL
  return configApiUrl("/v1")
}
