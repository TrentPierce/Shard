// Service Worker with Workbox for offline-first caching
const { precacheAndRoute, cleanupOutdatedCaches } = require('workbox-precaching')
const { offlineFallback, pagePattern } = require('workbox-recipes')
const { StaleWhileRevalidate } = require('workbox-strategies')

const CACHE_VERSION = 'v1'
const PRECACHE_ENTRIES = [
  '/',
  '/api/config',
]

const RUNTIME_CACHING = [
  new RegExp('\\.js$'),
  /\.css$/,
  /\/_next\/static\/.*/,
]

self.addEventListener('install', (event) => {
  console.log('[SW] Installing service worker')
  
  // Precache essential pages
  event.waitUntil(
    precacheAndRoute({
      cacheName: 'shard-v1',
      urls: PRECACHE_ENTRIES,
    }),
    cleanupOutdatedCaches({
      caches: ['shard-v1'],
      maxAgeSeconds: 7 * 24 * 60 * 60, // 7 days
    }),
    offlineFallback({
      url: '/api/offline',
      revision: CACHE_VERSION,
    })
  )
})

self.addEventListener('activate', (event) => {
  console.log('[SW] Service worker activated')
  
  // Clean up old caches on activation
  event.waitUntil(
    caches.keys().then((cacheNames) => {
      return Promise.all(
        cacheNames
          .map((cacheName) => {
            if (cacheName.startsWith('shard-') && cacheName !== `shard-${CACHE_VERSION}`) {
              return caches.delete(cacheName)
            }
          })
      )
    })
  )
})

// Network-first caching for API calls
self.addEventListener('fetch', (event) => {
  const { request } = event
  
  // Skip non-GET requests
  if (request.method !== 'GET') {
    event.respondWith(fetch(request))
    return
  }
  
  // Skip API requests when online (they'll go directly to Oracle)
  if (request.url.includes('/v1/') || request.url.includes('/api/')) {
    event.respondWith(fetch(request))
    return
  }
  
  // Handle offline with stale-while-revalidate strategy
  event.respondWith(
    new StaleWhileRevalidate({
      cacheName: 'shard-api-cache',
      plugins: [
        offlineFallback({
          url: '/api/offline',
          revision: CACHE_VERSION,
          precacheFallback: false,
        }),
      ],
    }).handle({ request })
  )
})

// Handle background sync of Oracle node list
self.addEventListener('message', (event) => {
  if (event.data && event.data.type === 'SYNC_ORACLE_NODES') {
    console.log('[SW] Syncing Oracle nodes list')
    
    // Fetch latest node list from configured bootstrap endpoints
    syncOracleNodes().catch((error) => {
      console.error('[SW] Failed to sync Oracle nodes:', error)
    })
  }
})

async function syncOracleNodes() {
  try {
    // Try multiple endpoints in order of preference
    const endpoints = [
      '/v1/system/topology',
      '/topology',
      'https://raw.githubusercontent.com/ShardNetwork/nodes/main.json',
    ]
    
    for (const endpoint of endpoints) {
      try {
        const response = await fetch(endpoint, { cache: 'no-store' })
        
        if (response.ok) {
          const nodes = await response.json()
          const cache = await caches.open('shard-nodes-cache')
          await cache.put('oracle-nodes', new Response(JSON.stringify(nodes), {
            headers: { 'Content-Type': 'application/json' }
          }))
          return
        }
      } catch (e) {
        console.warn(`[SW] Failed to fetch from ${endpoint}:`, e)
      }
    }
  } catch (error) {
    console.error('[SW] Error syncing Oracle nodes:', error)
  }
}

// Notify clients about cached updates
self.addEventListener('controllerchange', () => {
  console.log('[SW] Controller changed, reloading page')
})
